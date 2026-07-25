use std::sync::{Arc, Mutex};

use crate::{data_source::SourceContext, pcm_frames::PcmFormat, ErrorKinds, MaResult, MaudioError};

/// A source of interleaved PCM frames for a [`DataSource`](crate::data_source).
///
/// Implementors must provide [`PcmSource::fill_pcm_frames`]. The remaining
/// methods represent optional data-source capabilities.
///
/// Override an optional method to expose that capability through the resulting
/// [`DataSource`](crate::data_source). Leaving its default implementation unchanged causes the
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
    /// This method performs a single read from the source's current position.
    /// Looping, and data-source chaining are handled by miniaudio outside
    /// of this callback and should not normally be implemented here, unless
    /// the implementor specifically wants to manage that state and behaviour.
    ///
    /// The length of `out` is measured in PCM units rather than frames. The
    /// number of frames it can hold is therefore:
    ///
    /// ```text
    /// out.len() / ctx.data_format.channels
    /// ```
    ///
    /// Returns the number of complete PCM frames written to `out`. The returned
    /// count must not exceed the capacity of `out` for the configured channel
    /// count.
    ///
    /// Returning fewer frames than requested indicates that only that many
    /// frames were produced. Returning zero indicates that the source has
    /// reached its end. The unused portion of `out` does not need to be filled
    /// with silence.
    ///
    /// ### Cursor
    ///
    /// The implementation is responsible for keeping the cursor in `ctx`
    /// consistent with the frames it consumes.
    ///
    /// The cursor does must track frames, not individual samples.
    /// For example, consuming 10 stereo frames advances the cursor by 10 even
    /// though 20 samples were read from the underlying storage.
    ///
    /// ### Looping
    ///
    /// This method is not normally responsible for looping. When looping is
    /// enabled and this method reports the end of the source, miniaudio seeks
    /// to the configured loop beginning and continues reading.
    ///
    /// Miniaudio-managed looping therefore needs
    /// [`PcmSource::seek_to_pcm_frame`] to be implemented. The read
    /// implementation must also:
    ///
    /// - accurately report the number of frames produced;
    /// - advance the cursor by that number of frames;
    /// - eventually return zero after reaching the end of the source.
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
    /// This method is optional for sequential reading, but is required for:
    ///
    /// - seeking through the resulting [`DataSource`](crate::data_source);
    /// - miniaudio-managed looping;
    /// - entering this source through a data-source chain,
    ///   because miniaudio rewinds the next source to frame zero.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`].
    /// Consequently, operations which require repositioning may return
    /// `MA_NOT_IMPLEMENTED`, including:
    ///
    /// - [`DataSource::seek_pcm_frames`](crate::data_source);
    /// - [`DataSource::seek_to_pcm_frame`](crate::data_source);
    /// - [`DataSource::seek_seconds`](crate::data_source);
    /// - [`DataSource::seek_to_second`](crate::data_source).
    ///
    /// Not implementing this method does not prevent
    /// [`PcmSource::fill_pcm_frames`] from reading sequentially or maintaining
    /// [`SourceContext::cursor`].
    fn seek_to_pcm_frame(&mut self, _frame_index: u64, _ctx: &mut SourceContext) -> MaResult<()> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Returns the current cursor position in PCM frames.
    ///
    /// The returned position identifies the frame that will be read next.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// disabling cursor queries through the resulting [`DataSource`](crate::data_source).
    fn cursor_in_pcm_frames(&self, _ctx: &SourceContext) -> MaResult<u64> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Returns the total length of the source in PCM frames.
    ///
    /// The default implementation returns [`ErrorKinds::NotImplemented`],
    /// disabling length queries through the resulting [`DataSource`](crate::data_source).
    fn length_in_pcm_frames(&self, _ctx: &SourceContext) -> MaResult<u64> {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }

    /// Called when the data source's looping state is changed.
    ///
    /// Miniaudio stores and uses the looping state itself. This callback does not
    /// perform looping; it only allows the implementation to react to the new
    /// state or mirror it in its own internal state.
    ///
    /// The default implementation accepts the change without performing any
    /// additional work.
    ///
    /// The [`SourceContext::looping`] flag is updated automatically.
    fn on_looping(&mut self, _looping: bool, _ctx: &mut SourceContext) -> MaResult<()> {
        Ok(())
    }
}

impl<F: PcmFormat> PcmSource<F> for Vec<F::PcmUnit> {
    fn fill_pcm_frames(
        &mut self,
        out: &mut [F::PcmUnit],
        ctx: &mut SourceContext,
    ) -> MaResult<usize> {
        let channels = ctx.data_format.channels as usize;
        let length_frames = self.len() / channels;
        let cursor_frames = usize::try_from(ctx.cursor).unwrap_or(usize::MAX);

        if cursor_frames >= length_frames {
            return Ok(0);
        }

        let capacity_frames = out.len() / channels;
        let available_frames = length_frames - cursor_frames;
        let frames_to_copy = capacity_frames.min(available_frames);

        let src_start = cursor_frames * channels;
        let samples_to_copy = frames_to_copy * channels;

        out[..samples_to_copy].copy_from_slice(&self[src_start..src_start + samples_to_copy]);

        ctx.cursor += frames_to_copy as u64;

        Ok(frames_to_copy)
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
}
