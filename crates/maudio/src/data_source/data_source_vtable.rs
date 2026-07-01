use crate::{
    data_source::{data_source_builder::DataSourceBuilder, pcm_source::PcmSource, DataSourceInner},
    pcm_frames::PcmFormat,
    ErrorKinds,
};

use maudio_sys::ffi as sys;

pub(crate) fn data_source_vtable<F: PcmFormat, P: PcmSource<F>>(
    builder: &DataSourceBuilder,
) -> *const sys::ma_data_source_vtable {
    let mut vtable = sys::ma_data_source_vtable {
        onRead: Some(data_source_read_proc::<F, P>),
        onSeek: Some(data_source_seek_proc::<F, P>),
        onGetDataFormat: Some(data_source_get_format_proc::<F, P>),
        onGetCursor: Some(data_source_get_cursor_proc::<F, P>),
        onGetLength: Some(data_source_get_len_proc::<F, P>),
        onSetLooping: Some(data_source_set_looping_proc::<F, P>),
        flags: 0,
    };
    if builder.no_seek {
        vtable.onSeek = None;
    }
    if builder.no_cursor {
        vtable.onGetCursor = None;
        vtable.onSeek = None;
    }
    if builder.no_length {
        vtable.onGetLength = None;
    }
    if builder.no_looping {
        vtable.onSetLooping = None;
    }
    Box::into_raw(Box::new(vtable)) as *const _
}

unsafe extern "C" fn data_source_read_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    frames_out: *mut core::ffi::c_void,
    frame_count: u64,
    frames_read: *mut u64,
) -> sys::ma_result {
    if data_source.is_null() || frames_out.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    if !frames_read.is_null() {
        *frames_read = 0;
    }

    if frame_count == 0 {
        return sys::ma_result_MA_SUCCESS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();
    let slice_len =
        match (frame_count as usize).checked_mul(ds.context.data_format.channels as usize) {
            Some(len) => len,
            None => return sys::ma_result_MA_INVALID_ARGS,
        };

    // Safety:
    // The output slice expects F::StorageUnit here
    // When format is S24 StorageUnit and PcmUnit are different, but S24 is not allowed by the API
    // PcmUnit and StorageUnit always the same layout, size, and alignment
    let out = core::slice::from_raw_parts_mut::<F::PcmUnit>(frames_out.cast(), slice_len);

    match ds.source.fill_pcm_frames(out, &mut ds.context) {
        Ok(frames) => {
            if !frames_read.is_null() {
                *frames_read = frames as u64;
            }

            ds.context.cursor = ds.context.cursor.saturating_add(frames as u64);

            if frames == 0 {
                sys::ma_result_MA_AT_END
            } else {
                sys::ma_result_MA_SUCCESS
            }
        }
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn data_source_seek_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    frame_index: u64,
) -> sys::ma_result {
    if data_source.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();

    match ds.source.seek_to_pcm_frame(frame_index, &mut ds.context) {
        Ok(_) => sys::ma_result_MA_SUCCESS,
        Err(e) if e.kind() == Some(&ErrorKinds::NotImplemented) => {
            sys::ma_result_MA_NOT_IMPLEMENTED
        }
        Err(_) => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn data_source_get_format_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    format: *mut sys::ma_format,
    channels: *mut u32,
    sample_rate: *mut u32,
    channel_map: *mut sys::ma_channel,
    channel_map_cap: usize,
) -> sys::ma_result {
    if data_source.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();

    if !format.is_null() {
        *format = ds.context.data_format.format.into();
    }

    if !channels.is_null() {
        *channels = ds.context.data_format.channels;
    }

    if !sample_rate.is_null() {
        *sample_rate = ds.context.data_format.sample_rate.into();
    }

    if !channel_map.is_null() && !channel_map_cap > 0 {
        if let Some(map) = ds.context.data_format.channel_map.as_ref() {
            let count = core::cmp::min(map.len(), channel_map_cap);

            core::ptr::copy_nonoverlapping(map.as_ptr(), channel_map.cast(), count);
        }
    }

    sys::ma_result_MA_SUCCESS
}

unsafe extern "C" fn data_source_get_cursor_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    cursor: *mut u64,
) -> sys::ma_result {
    if data_source.is_null() || cursor.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();

    *cursor = ds.context.cursor;
    sys::ma_result_MA_SUCCESS
}

unsafe extern "C" fn data_source_get_len_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    length: *mut u64,
) -> sys::ma_result {
    if data_source.is_null() || length.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();

    let len = ds.source.length_in_pcm_frames(&ds.context).unwrap_or(0);
    *length = len;
    sys::ma_result_MA_SUCCESS
}

unsafe extern "C" fn data_source_set_looping_proc<F: PcmFormat, P: PcmSource<F>>(
    data_source: *mut sys::ma_data_source,
    is_looping: u32,
) -> sys::ma_result {
    if data_source.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    let ds = &mut *(data_source).cast::<DataSourceInner<F, P>>();

    if ds
        .source
        .set_looping(is_looping == 1, &mut ds.context)
        .is_err()
    {
        return sys::ma_result_MA_NOT_IMPLEMENTED;
    }

    sys::ma_result_MA_SUCCESS
}
