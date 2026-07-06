use std::panic::AssertUnwindSafe;

use maudio_sys::ffi as sys;

use crate::engine::node_graph::{
    node_builder::NodeFunction,
    node_flags::NodeFlags,
    node_on_process::{CustomNode, InputBusses, OutputBusses, ReqFramesNode},
    nodes::NodeInner,
};

pub(crate) fn node_vtable<C: CustomNode>(
    in_bus: u8,
    out_bus: u8,
    flags: NodeFlags,
) -> *const sys::ma_node_vtable {
    let vtable = sys::ma_node_vtable {
        onProcess: Some(node_on_process::<C>),
        onGetRequiredInputFrameCount: None,
        inputBusCount: in_bus,
        outputBusCount: out_bus,
        flags: flags.bits(),
    };

    Box::into_raw(Box::new(vtable)) as *const _
}

pub(crate) fn node_vtable_req_frames<C: CustomNode + ReqFramesNode>(
    in_bus: u8,
    out_bus: u8,
    flags: NodeFlags,
) -> *const sys::ma_node_vtable {
    let vtable = sys::ma_node_vtable {
        onProcess: Some(node_on_process::<C>),
        onGetRequiredInputFrameCount: Some(node_on_required_input_frame_count::<C>),
        inputBusCount: in_bus,
        outputBusCount: out_bus,
        flags: flags.bits(),
    };

    Box::into_raw(Box::new(vtable)) as *const _
}

unsafe extern "C" fn node_on_process<C: CustomNode>(
    node: *mut sys::ma_node,
    frames_in: *mut *const f32,
    frame_count_in: *mut u32,
    frames_out: *mut *mut f32,
    frame_count_out: *mut u32,
) {
    if node.is_null() {
        return;
    }

    let node = &mut *(node).cast::<NodeInner<C>>();
    let flags = NodeFlags::from_bits((*node.vtable).flags);

    match node.op {
        NodeFunction::Source => {
            if frames_out.is_null() || frame_count_out.is_null() {
                return;
            }
            let mut output =
                OutputBusses::from_raw(frames_out, *frame_count_out as usize, &node.busses.outputs);
            // We do not need to update frame_count_in or frame_count_out. We can ignore the output.
            let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                node.custom
                    .process_frames(&InputBusses::zeroed(), &mut output)
            }));
            match res {
                Ok(Ok(frames)) => {
                    let frames_output = frames.frames_out_written;
                    if frames_output > *frame_count_out {
                        *frame_count_out = 0;
                        return;
                    }
                    if flags.contains(NodeFlags::PASSTHROUGH) {
                        // If in passthrough, silence the buffer again in case the user modified it for whatever reason
                        // Leave frame_count_out unchanged
                        if let Some(buff) = output.get_mut_bus(0) {
                            buff.fill(0.0)
                        };
                        return;
                    }
                    *frame_count_out = frames_output;
                }
                _ => {
                    *frame_count_out = 0;
                }
            }
        }
        NodeFunction::Passthrough => {
            if frames_in.is_null() || frame_count_in.is_null() {
                return;
            }
            let input =
                InputBusses::from_raw(frames_in, *frame_count_in as usize, &node.busses.inputs);

            // We do not need to update frame_count_in or frame_count_out. We can ignore the output.
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                node.custom
                    .process_frames(&input, &mut OutputBusses::zeroed())
            }));
        }
        NodeFunction::Process => {
            if frames_out.is_null() || frame_count_out.is_null() {
                return;
            }
            let frames_to_process = (*frame_count_in).min(*frame_count_out);

            let input = if frames_in.is_null() {
                if flags.contains(NodeFlags::ALLOW_NULL_INPUT) {
                    let mut input = InputBusses::zeroed();
                    input.is_null = true;
                    input
                } else {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                    return;
                }
            } else {
                if frame_count_in.is_null() {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                    return;
                }

                InputBusses::from_raw(frames_in, frames_to_process as usize, &node.busses.inputs)
            };

            let mut output = OutputBusses::from_raw(
                frames_out,
                frames_to_process as usize,
                &node.busses.outputs,
            );

            let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                node.custom.process_frames(&input, &mut output)
            }));
            match res {
                Ok(Ok(frames)) => {
                    // The 2 fields inside ProcessResult are equal
                    let frames = frames.frames_out_written;
                    if frames > *frame_count_out || frames > *frame_count_in {
                        // User has probably returned number of samples instead of frames.
                        *frame_count_in = 0;
                        *frame_count_out = 0;
                    }
                    *frame_count_in = frames;
                    *frame_count_out = frames;
                }
                _ => {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                }
            }
        }
        NodeFunction::Resampler => {
            if frames_out.is_null() || frame_count_out.is_null() {
                return;
            }

            let input = if frames_in.is_null() {
                if flags.contains(NodeFlags::ALLOW_NULL_INPUT) {
                    let mut input = InputBusses::zeroed();
                    input.is_null = true;
                    input
                } else {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                    return;
                }
            } else {
                if frame_count_in.is_null() {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                    return;
                }
                InputBusses::from_raw(frames_in, *frame_count_in as usize, &node.busses.inputs)
            };

            let mut output =
                OutputBusses::from_raw(frames_out, *frame_count_out as usize, &node.busses.outputs);
            let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                node.custom.process_frames(&input, &mut output)
            }));
            match res {
                Ok(Ok(frames)) => {
                    // The 2 fields inside ProcessResult are equal
                    let frames_read = frames.frames_in_consumed;
                    let frames_written = frames.frames_out_written;
                    if frames_written > *frame_count_out || frames_read > *frame_count_in {
                        // User has probably returned number of samples instead of frames.
                        *frame_count_in = 0;
                        *frame_count_out = 0;
                    }
                    *frame_count_in = frames_read;
                    *frame_count_out = frames_written;
                }
                _ => {
                    *frame_count_in = 0;
                    *frame_count_out = 0;
                }
            }
        }
    }
}

unsafe extern "C" fn node_on_required_input_frame_count<C: CustomNode + ReqFramesNode>(
    node: *mut sys::ma_node,
    out_frame_count: u32,
    in_frame_count: *mut u32,
) -> sys::ma_result {
    if node.is_null() || in_frame_count.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let node = &mut *(node).cast::<NodeInner<C>>();

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        node.custom.get_required_frames(out_frame_count)
    }));

    match res {
        Ok(Ok(frames)) => {
            *in_frame_count = frames;
            sys::ma_result_MA_SUCCESS
        }
        _ => {
            *in_frame_count = 0;
            sys::ma_result_MA_ERROR
        }
    }
}
