use crate::MaResult;

/// Processing callback for sink custom nodes.
///
/// A sink processor receives input audio but does not write output audio.
/// This is useful for nodes that inspect, meter, analyze, or consume audio from
/// the graph.
///
/// The `input` slice contains interleaved `f32` PCM samples for the node's input
/// bus. The number of frames represented by the slice depends on the channel
/// count of the bus.
///
/// Returning an error causes processing for this callback invocation to fail.
pub trait SinkCallback {
    /// Processes input audio for a sink node.
    fn on_audio(&mut self, input: &[f32]) -> MaResult<()>;
}

/// Processing callback for source custom nodes.
///
/// A source processor generates audio without receiving input audio from the
/// graph. This is useful for oscillators, procedural generators, decoders, or
/// any node that produces audio from its own state.
///
/// The `output` slice contains interleaved `f32` PCM samples for the node's
/// output bus. The processor may write up to the full length of this buffer.
///
/// The returned frame count tells the node graph how many PCM frames were
/// produced. This count must not exceed the output capacity for the callback.
pub trait SourceCallback {
    /// Writes output audio for a source node.
    fn on_audio(&mut self, output: &mut [f32]) -> MaResult<u32>;
}

/// Processing callback for effect (processor) custom nodes.
///
/// An effect receives input audio and writes output audio at the same
/// processing rate. This is useful for effects, filters, mixers, analyzers that
/// also forward audio, and other input-to-output effects.
///
/// `input` provides access to the node's input busses. `output` provides mutable
/// access to the node's output busses.
///
/// The processor may read from any available input bus. For every frame it
/// reports as written, it should fully initialize the corresponding frames in
/// every available output bus.
///
/// The returned frame count tells the node graph how many PCM frames were
/// produced. For normal effect nodes, this should usually be no greater than
/// the number of input frames available and must not exceed the output capacity
/// for the callback.
pub trait EffectCallback {
    /// Processes input audio and writes output audio.
    fn on_audio(&mut self, input: &InputBusses, output: &mut OutputBusses) -> MaResult<u32>;
}

/// Processing callback for transform (resampling) custom nodes.
///
/// A transform processor receives input audio and writes output audio when the
/// number of input frames consumed may differ from the number of output frames
/// produced. This is useful for sample-rate conversion, time stretching, or
/// other processors that operate at different input and output rates.
///
/// `input` provides access to the node's input busses. `output` provides mutable
/// access to the node's output busses.
///
/// The returned [`ProcessResult`] reports both how many input frames were
/// consumed and how many output frames were produced. The consumed input frame
/// count must not exceed the available input frame count, and the produced
/// output frame count must not exceed the output capacity for the callback.
pub trait TransformCallback {
    /// Processes input and output audio with independent frame counts.
    fn on_audio(
        &mut self,
        input: &InputBusses,
        output: &mut OutputBusses,
    ) -> MaResult<ProcessResult>;
}

/// Reports how much input a transform node needs for a requested amount of output.
///
/// This callback is used by transform nodes whose input and output frame counts do
/// not necessarily match. The most common example is a resampler: producing
/// `out_frames` output frames may require more, fewer, or a fractional number of
/// input frames depending on the conversion ratio and any internal filter history.
///
/// Miniaudio uses this value as a scheduling hint when pulling data through the
/// node graph. When a downstream node asks this node for some number of output
/// frames, miniaudio may call `required_input_frames()` first to decide how many
/// frames to read from this node's inputs before calling the transform callback.
///
/// This does **not** replace the transform callback's normal frame-count contract.
/// The transform callback must still report how many input frames it actually
/// consumed and how many output frames it actually produced.
///
/// Returning an error means the input demand could not be calculated. Miniaudio
/// treats the native callback as optional and uses it as a hint, so you should
/// not rely on this callback being the only enforcement mechanism for correctness.
/// The transform callback must still handle the actual amount of input it receives.
pub trait InputDemandCallback {
    fn required_input_frames(&mut self, out_frames: u32) -> MaResult<u32>;
}

pub struct InputBusses<'a> {
    ptrs: &'a mut [*const f32],
    frames_per_bus: usize,
    channels_per_bus: &'a [u32],
    pub(crate) is_null: bool,
}

impl<'a> InputBusses<'a> {
    pub(crate) fn zeroed() -> Self {
        Self {
            ptrs: &mut [],
            frames_per_bus: 0,
            channels_per_bus: &[],
            is_null: false,
        }
    }

    pub(crate) unsafe fn from_raw(
        frames_out: *mut *const f32,
        frames_per_bus: usize,
        channels_per_bus: &'a [u32],
    ) -> Self {
        let ptrs: &mut [*const f32] =
            std::slice::from_raw_parts_mut(frames_out, channels_per_bus.len());

        Self {
            ptrs,
            frames_per_bus,
            channels_per_bus,
            is_null: false,
        }
    }

    pub fn len(&self) -> usize {
        self.ptrs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ptrs.is_empty()
    }

    /// Only useful is `allow_null_input` was used when building the Node
    pub fn null_input(&self) -> bool {
        self.is_null
    }

    /// Retrieve the channel count for a specific bus
    ///
    /// Returns None is bus index is out of range
    pub fn get_channels(&self, bus_index: usize) -> Option<u32> {
        self.channels_per_bus.get(bus_index).copied()
    }

    /// Retrieve interleaved PCM samples for a specific bus
    ///
    /// Returns None is bus index is out of range
    pub fn get_bus(&self, bus_index: usize) -> Option<&[f32]> {
        let ptr = *self.ptrs.get(bus_index)?;

        if ptr.is_null() {
            return None;
        }

        let channels = *self.channels_per_bus.get(bus_index)?;
        let samples = self.frames_per_bus * channels as usize;

        Some(unsafe { std::slice::from_raw_parts(ptr, samples) })
    }
}

pub struct OutputBusses<'a> {
    ptrs: &'a mut [*mut f32],
    frames_per_bus: usize,
    channels_per_bus: &'a [u32],
}

impl<'a> OutputBusses<'a> {
    pub(crate) fn zeroed() -> Self {
        Self {
            ptrs: &mut [],
            frames_per_bus: 0,
            channels_per_bus: &[],
        }
    }

    pub(crate) unsafe fn from_raw(
        frames_out: *mut *mut f32,
        frames_per_bus: usize,
        channels_per_bus: &'a [u32],
    ) -> Self {
        let ptrs = std::slice::from_raw_parts_mut(frames_out, channels_per_bus.len());

        Self {
            ptrs,
            frames_per_bus,
            channels_per_bus,
        }
    }

    pub fn len(&self) -> usize {
        self.ptrs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ptrs.is_empty()
    }

    /// Retrieve the channel count for a specific bus
    ///
    /// Returns None is bus index is out of range
    pub fn get_channels(&self, bus_index: usize) -> Option<u32> {
        self.channels_per_bus.get(bus_index).copied()
    }

    /// Retrieve mutable interleaved PCM samples for a specific bus
    ///
    /// Returns None is bus index is out of range
    pub fn get_mut_bus(&mut self, bus_index: usize) -> Option<&mut [f32]> {
        let ptr = *self.ptrs.get_mut(bus_index)?;

        if ptr.is_null() {
            return None;
        }

        let channels = *self.channels_per_bus.get(bus_index)?;
        let samples = self.frames_per_bus * channels as usize;

        Some(unsafe { std::slice::from_raw_parts_mut(ptr, samples) })
    }
}

/// Result of a processing callback with independent input and output frame counts.
///
/// This is used by callbacks where the number of input frames consumed may
/// differ from the number of output frames written.
///
/// Both counts are measured in PCM frames, not samples. For interleaved audio,
/// one frame contains one sample per channel.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessResult {
    /// Number of input PCM frames consumed by the callback.
    ///
    /// This must not exceed the number of input frames made available to the
    /// callback.
    pub frames_in_consumed: u32,

    /// Number of output PCM frames written by the callback.
    ///
    /// This must not exceed the output frame capacity made available to the
    /// callback.
    pub frames_out_written: u32,
}

pub trait CustomNode {
    type Inner;

    fn inner(&self) -> &Self::Inner;
    fn inner_mut(&mut self) -> &mut Self::Inner;

    fn process_frames<'a>(
        &mut self,
        input: &InputBusses<'a>,
        output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult>;
}

pub struct Sink<P>(pub(crate) P);

impl<P: SinkCallback> CustomNode for Sink<P> {
    type Inner = P;

    fn inner(&self) -> &Self::Inner {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    fn process_frames<'a>(
        &mut self,
        input: &InputBusses<'a>,
        _output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult> {
        // Safety:
        // We are guaranteed to only have one input bus on index 0
        self.0.on_audio(input.get_bus(0).unwrap())?;
        Ok(ProcessResult::default())
    }
}

pub struct Source<P>(pub(crate) P);

impl<P: SourceCallback> CustomNode for Source<P> {
    type Inner = P;

    fn inner(&self) -> &Self::Inner {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    fn process_frames<'a>(
        &mut self,
        _input: &InputBusses<'a>,
        output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult> {
        // Safety:
        // We are guaranteed to only have one outpus bus on index 0
        let frames = self.0.on_audio(output.get_mut_bus(0).unwrap())?;
        Ok(ProcessResult {
            frames_in_consumed: frames,
            frames_out_written: frames,
        })
    }
}

pub struct Effect<P>(pub(crate) P);

impl<P: EffectCallback> CustomNode for Effect<P> {
    type Inner = P;

    fn inner(&self) -> &Self::Inner {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    fn process_frames<'a>(
        &mut self,
        input: &InputBusses<'a>,
        output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult> {
        let proc_frames = self.0.on_audio(input, output)?;
        Ok(ProcessResult {
            frames_in_consumed: proc_frames,
            frames_out_written: proc_frames,
        })
    }
}

pub struct Transform<P>(pub(crate) P);

impl<P: TransformCallback> CustomNode for Transform<P> {
    type Inner = P;

    fn inner(&self) -> &Self::Inner {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    fn process_frames<'a>(
        &mut self,
        input: &InputBusses<'a>,
        output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult> {
        self.0.on_audio(input, output)
    }
}

pub trait ReqFramesNode {
    fn get_required_frames(&mut self, out_frames: u32) -> MaResult<u32>;
}

pub struct TransformInputDemand<P>(pub(crate) P);

impl<P: TransformCallback> CustomNode for TransformInputDemand<P> {
    type Inner = P;

    fn inner(&self) -> &Self::Inner {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
    }

    fn process_frames<'a>(
        &mut self,
        input: &InputBusses<'a>,
        output: &mut OutputBusses<'a>,
    ) -> MaResult<ProcessResult> {
        self.0.on_audio(input, output)
    }
}

impl<P: TransformCallback + InputDemandCallback> ReqFramesNode for TransformInputDemand<P> {
    fn get_required_frames(&mut self, out_frames: u32) -> MaResult<u32> {
        self.0.required_input_frames(out_frames)
    }
}
