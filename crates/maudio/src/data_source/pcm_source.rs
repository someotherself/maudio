#![allow(unused_variables)]

use std::sync::{Arc, Mutex};

use crate::{data_source::SourceContext, pcm_frames::PcmFormat, ErrorKinds, MaResult, MaudioError};

pub trait PcmSource<F: PcmFormat> {
    fn fill_pcm_frames(&self, out: &mut [F::PcmUnit], ctx: &mut SourceContext) -> MaResult<usize>;

    fn seek_to_pcm_frame(&self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()>;

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64>;

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64>;

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()>;
}

impl<F: PcmFormat> PcmSource<F> for Vec<F::PcmUnit> {
    fn fill_pcm_frames(&self, out: &mut [F::PcmUnit], ctx: &mut SourceContext) -> MaResult<usize> {
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

    fn seek_to_pcm_frame(&self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        let cursor_samples = frame_index * ctx.data_format.channels as u64;
        if cursor_samples > self.len() as u64 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Trying to seek too far",
            )));
        }
        ctx.cursor = frame_index;
        Ok(())
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        Some(ctx.cursor)
    }

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        Some((self.len() as u64) / ctx.data_format.channels as u64)
    }

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()> {
        ctx.looping = looping;
        Ok(())
    }
}

impl<F, S> PcmSource<F> for Arc<S>
where
    F: PcmFormat,
    S: PcmSource<F> + ?Sized,
{
    fn fill_pcm_frames(&self, out: &mut [F::PcmUnit], ctx: &mut SourceContext) -> MaResult<usize> {
        (**self).fill_pcm_frames(out, ctx)
    }

    fn seek_to_pcm_frame(&self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        (**self).seek_to_pcm_frame(frame_index, ctx)
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        (**self).cursor_in_pcm_frames(ctx)
    }

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        (**self).length_in_pcm_frames(ctx)
    }

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()> {
        (**self).set_looping(looping, ctx)
    }
}

impl<F, S> PcmSource<F> for Mutex<S>
where
    F: PcmFormat,
    S: PcmSource<F>,
{
    fn fill_pcm_frames(&self, out: &mut [F::PcmUnit], ctx: &mut SourceContext) -> MaResult<usize> {
        let src = self.lock().unwrap();
        (*src).fill_pcm_frames(out, ctx)
    }

    fn seek_to_pcm_frame(&self, frame_index: u64, ctx: &mut SourceContext) -> MaResult<()> {
        let src = self.lock().unwrap();
        (*src).seek_to_pcm_frame(frame_index, ctx)
    }

    fn cursor_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        let src = self.lock().unwrap();
        (*src).cursor_in_pcm_frames(ctx)
    }

    fn length_in_pcm_frames(&self, ctx: &SourceContext) -> Option<u64> {
        let src = self.lock().unwrap();
        (*src).length_in_pcm_frames(ctx)
    }

    fn set_looping(&self, looping: bool, ctx: &mut SourceContext) -> MaResult<()> {
        let src = self.lock().unwrap();
        (*src).set_looping(looping, ctx)
    }
}
