use std::sync::{Arc, Mutex};

use crate::{data_source::SourceContext, pcm_frames::PcmFormat, ErrorKinds, MaResult, MaudioError};

/// A source of interleaved PCM frames for a [`DataSource`].
///
/// Implementors must provide [`PcmSource::fill_pcm_frames`]. The remaining
/// methods represent optional data-source capabilities and return
/// [`ErrorKinds::NotImplemented`] by default.
///
/// Override an optional method to expose that capability through the resulting
/// [`DataSource`]. Leaving its default implementation unchanged causes the
/// corresponding `DataSource` operations to return `MA_NOT_IMPLEMENTED`.
///
/// [`SourceContext`] contains information managed by the data source, including
/// its format, cursor position, and looping state. Implementations may inspect
/// and update this context while handling an operation.
///
/// For examples, see the existing implementations for this trait.
pub trait PcmSource<F: PcmFormat> {
    /// Reads PCM frames into `out`.
    ///
    /// `out` contains interleaved PCM samples in the format represented by `F`.
    /// Its length is measured in PCM units, not frames. The number of requested
    /// frames can be calculated from the channel count in `ctx`.
    ///
    /// Returns the number of complete PCM frames written to `out`. This must
    /// not exceed the capacity of `out` for the configured channel count.
    /// Returning fewer frames than requested indicates that only that many
    /// frames were currently available. Returning zero indicates that no
    /// frames were produced.
    ///
    /// The implementation is responsible for keeping the cursor in `ctx`
    /// consistent with the frames it consumes.
    ///
    /// When looping is enabled, the implementation must perform any necessary
    /// rewind and continue filling `out` itself.
    fn fill_pcm_frames(
        &mut self,
        out: &mut [F::PcmUnit],
        ctx: &mut SourceContext,
    ) -> MaResult<usize>;

    /// Seeks to an absolute PCM frame.
    ///
    /// After a successful seek, the next call to
    /// [`PcmSource::fill_pcm_frames`] must begin at `frame_index`.
    ///
    /// The implementation is responsible for keeping the cursor in `ctx`
    /// consistent with the new position.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// disabling seeking through the resulting [`DataSource`].
    ///
    /// When seeking is disabled, the following methods return `MA_NOT_IMPLEMENTED`:
    /// - `DataSource::seek_pcm_frames`
    /// - `DataSource::seek_to_pcm_frame`
    /// - `DataSource::seek_seconds`
    /// - `DataSource::seek_to_second`
    ///   This will affect other components in audio chain connected to this data source,
    ///   when they try to access these methods.
    ///
    /// However, it does not prevent the user implementation of
    /// [`PcmSource::fill_pcm_frames`] from using or modifying the cursor, via `ctx.cursor`.
    ///
    /// This may also be useful when the `PcmSource` type self manages the cursor.
    ///
    /// The [`PcmSource::seek_to_pcm_frame`] will never run and may return `MaudioError::NotImplemented`.
    fn seek_to_pcm_frame(&mut self, _frame_index: u64, _ctx: &mut SourceContext) -> MaResult<()> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Returns the current cursor position in PCM frames.
    ///
    /// The returned position identifies the frame that will be read next.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// disabling cursor queries through the resulting [`DataSource`].
    fn cursor_in_pcm_frames(&self, _ctx: &SourceContext) -> MaResult<u64> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Returns the total length of the source in PCM frames.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// disabling length queries through the resulting [`DataSource`].
    fn length_in_pcm_frames(&self, _ctx: &SourceContext) -> MaResult<u64> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Enables or disables the ability for this source to loop.
    ///
    /// When `looping` is `true`, subsequent calls to
    /// [`PcmSource::fill_pcm_frames`] must repeat the source instead of remaining
    /// at the end. The source implementation is responsible for rewinding and
    /// producing the repeated frames.
    ///
    /// `ctx` exposes the data source's current state and may be updated as needed.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// meaning that the resulting [`DataSource`] does not support changing its
    /// looping behavior.
    fn set_looping(&self, _looping: bool, _ctx: &mut SourceContext) -> MaResult<()> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}

impl<F: PcmFormat> PcmSource<F> for Vec<F::PcmUnit> {
    fn fill_pcm_frames(
        &mut self,
        out: &mut [F::PcmUnit],
        ctx: &mut SourceContext,
    ) -> MaResult<usize> {
        let channels = ctx.data_format.channels as usize;

        let cursor_samples = ctx.cursor as usize * channels;

        // seek_to_pcm_frame should also defend against this
        if cursor_samples > self.len() {
            out.fill(F::PCM_UNIT_SILENCE);
            return Ok(0);
        }

        let mut samples_written = 0;
        let capacity_samples = out.len();

        loop {
            let remaining_capacity_samples = out.len() - samples_written;

            let cursor_samples = ctx.cursor as usize * channels;

            let available_samples = (self.len()).saturating_sub(cursor_samples);
            // Make sure we only copy whole frames. out.len() is guarateed to fit whole frames but not self.len()
            let samples_to_copy =
                available_samples.min(remaining_capacity_samples) / channels * channels;
            if samples_to_copy == 0 {
                out[samples_written..].fill(F::PCM_UNIT_SILENCE);
                break;
            }

            let src_start = cursor_samples;
            let src_end = cursor_samples + samples_to_copy;

            let out_start = samples_written;
            let out_end = out_start + samples_to_copy;
            out[out_start..out_end].copy_from_slice(&self[src_start..src_end]);

            samples_written += samples_to_copy;
            // Advance the cursor. The cursor keeps track of frames, not samples.
            ctx.cursor += (samples_to_copy / channels) as u64;

            if samples_written == capacity_samples {
                break;
            }

            // Check if we have reached the end of the source
            if ctx.cursor as usize * channels >= self.len() {
                if ctx.looping {
                    ctx.cursor = 0;
                    continue;
                } else {
                    // We have reached the end and looping is not enabled.
                    out[samples_written..capacity_samples].fill(F::PCM_UNIT_SILENCE);
                    break;
                }
            }
        }

        Ok(samples_written / channels)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        let cursor_samples = frame_index * ctx.data_format.channels as u64;
        if cursor_samples > self.len() as u64 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Trying to seek too far",
            )));
        }
        ctx.cursor = frame_index;
        Ok(())
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> MaResult<u64> {
        Ok(ctx.cursor)
    }

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> MaResult<u64> {
        Ok((self.len() as u64) / ctx.data_format.channels as u64)
    }

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()> {
        ctx.looping = looping;
        Ok(())
    }
}

impl<F, S> PcmSource<F> for Arc<Mutex<S>>
where
    F: PcmFormat,
    S: PcmSource<F>,
{
    fn fill_pcm_frames(
        &mut self,
        out: &mut [F::PcmUnit],
        ctx: &mut SourceContext,
    ) -> MaResult<usize> {
        let mut src = self.lock().unwrap();
        (*src).fill_pcm_frames(out, ctx)
    }

    fn seek_to_pcm_frame(&mut self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        let mut src = self.lock().unwrap();
        (*src).seek_to_pcm_frame(frame_index, ctx)
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> MaResult<u64> {
        let src = self.lock().unwrap();
        (*src).cursor_in_pcm_frames(ctx)
    }

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> MaResult<u64> {
        let src = self.lock().unwrap();
        (*src).length_in_pcm_frames(ctx)
    }

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()> {
        let src = self.lock().unwrap();
        (*src).set_looping(looping, ctx)
    }
}
