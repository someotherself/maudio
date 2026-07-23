use std::{
    ffi::CStr,
    fs::OpenOptions,
    io::{Cursor, Read, Seek},
    marker::PhantomData,
    mem::MaybeUninit,
    panic::AssertUnwindSafe,
    path::PathBuf,
};

use maudio_sys::ffi as sys;

use crate::{
    data_source::{
        data_source_builder::DataSourceBuilder,
        data_source_ffi,
        data_source_vtable::data_source_vtable,
        sources::decoder::{
            custom_decoder::{BackendDataSource, BackendRegistration},
            decoding_backend::{
                DecoderByteStream, DecoderFileStream, DecoderStream, DecodingBackend,
            },
        },
        DataFormat, SourceContext,
    },
    pcm_frames::PcmFormat,
    MaResult, MaudioError,
};

pub(crate) fn decoder_vtable<F: PcmFormat, D: DecodingBackend<Format = F>>(
) -> *const sys::ma_decoding_backend_vtable {
    let mut vtable: sys::ma_decoding_backend_vtable = sys::ma_decoding_backend_vtable {
        onInit: Some(decoder_on_init::<F, D>),
        onInitFile: None,
        onInitFileW: None,
        onInitMemory: Some(decoder_on_init_memory::<F, D>),
        onUninit: Some(decoder_on_uninit::<F, D>),
    };

    #[cfg(windows)]
    {
        vtable.onInitFileW = Some(decoder_on_init_file_w::<F, D>);
    }

    #[cfg(not(windows))]
    {
        vtable.onInitFile = Some(decoder_on_init_file::<F, D>);
    }
    Box::into_raw(Box::new(vtable)) as *const _
}

unsafe extern "C" fn decoder_on_init<F: PcmFormat, D: DecodingBackend<Format = F>>(
    backend_user_data: *mut core::ffi::c_void,
    on_read: sys::ma_read_proc,
    on_seek: sys::ma_seek_proc,
    on_tell: sys::ma_tell_proc,
    stream_user_data: *mut core::ffi::c_void,
    _: *const sys::ma_decoding_backend_config,
    _: *const sys::ma_allocation_callbacks,
    backend: *mut *mut sys::ma_data_source,
) -> sys::ma_result {
    if backend_user_data.is_null() || backend.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Don't leave this uninitialized
    unsafe {
        backend.write(core::ptr::null_mut());
    }

    let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // Keep the registratio, but remove the backend
        // Also, the vtable should be used?
        let registration = unsafe { &mut *backend_user_data.cast::<BackendRegistration<F>>() };

        let decoder_stream = DecoderStream {
            on_read,
            on_seek,
            on_tell,
            stream_user_data,
            cursor: 0,
        };

        let inner_ptr: *mut BackendDataSource<F, D> =
            create_data_source::<F, D, DecoderStream>(decoder_stream, registration)?;

        backend.write(inner_ptr.cast::<sys::ma_data_source>());

        Ok::<_, MaudioError>(())
    }));

    match res {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(e)) => e.ma_result.0,
        _ => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn decoder_on_init_file<F: PcmFormat, D: DecodingBackend<Format = F>>(
    backend_user_data: *mut core::ffi::c_void,
    path: *const core::ffi::c_char,
    _: *const sys::ma_decoding_backend_config,
    _: *const sys::ma_allocation_callbacks,
    backend: *mut *mut sys::ma_data_source,
) -> sys::ma_result {
    if backend_user_data.is_null() || path.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Don't leave this uninitialized
    unsafe {
        backend.write(core::ptr::null_mut());
    }

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { CStr::from_ptr(path) };
        let path = PathBuf::from(path.to_str().unwrap_or(""));

        let file = OpenOptions::new().read(true).open(&path)?;

        let decoder_stream = DecoderFileStream { file };

        let registration: &BackendRegistration<F> =
            unsafe { &*backend_user_data.cast::<BackendRegistration<F>>() };

        let inner_ptr =
            create_data_source::<F, D, DecoderFileStream>(decoder_stream, registration)?;

        backend.write(inner_ptr.cast::<sys::ma_data_source>());

        Ok::<_, MaudioError>(())
    }));

    match result {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(e)) => e.ma_result.0,
        _ => sys::ma_result_MA_ERROR,
    }
}

#[cfg(windows)]
unsafe extern "C" fn decoder_on_init_file_w<F: PcmFormat, D: DecodingBackend<Format = F>>(
    backend_user_data: *mut core::ffi::c_void,
    path: *const sys::wchar_t,
    _: *const sys::ma_decoding_backend_config,
    _: *const sys::ma_allocation_callbacks,
    backend: *mut *mut sys::ma_data_source,
) -> sys::ma_result {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    if backend_user_data.is_null() || path.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Don't leave this uninitialized
    unsafe {
        backend.write(core::ptr::null_mut());
    }

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut len = 0;
        while unsafe { *path.add(len) } != 0 {
            len += 1;
        }

        let wide = unsafe { std::slice::from_raw_parts(path, len) };
        let path = PathBuf::from(OsString::from_wide(wide));

        let file = OpenOptions::new().read(true).open(&path)?;

        let decoder_stream = DecoderFileStream { file };

        let registration: &BackendRegistration<F> =
            unsafe { &*backend_user_data.cast::<BackendRegistration<F>>() };

        let inner_ptr =
            create_data_source::<F, D, DecoderFileStream>(decoder_stream, registration)?;

        backend.write(inner_ptr.cast::<sys::ma_data_source>());

        Ok::<_, MaudioError>(())
    }));

    match result {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(e)) => e.ma_result.0,
        _ => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn decoder_on_init_memory<F: PcmFormat, D: DecodingBackend<Format = F>>(
    backend_user_data: *mut core::ffi::c_void,
    data: *const core::ffi::c_void,
    data_size: usize,
    _: *const sys::ma_decoding_backend_config,
    _: *const sys::ma_allocation_callbacks,
    backend: *mut *mut sys::ma_data_source,
) -> sys::ma_result {
    if backend_user_data.is_null() || data.is_null() {
        return sys::ma_result_MA_INVALID_ARGS;
    }

    // Don't leave this uninitialized
    unsafe {
        backend.write(core::ptr::null_mut());
    }

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // This slice is kept alive by the `CustomDecoder`
        // struct created by `CustomDecoderBuilder::from_memory`
        let slice: &[u8] = std::slice::from_raw_parts(data.cast(), data_size);

        let decoder_stream = DecoderByteStream {
            bytes: Cursor::new(slice),
        };

        let registration: &BackendRegistration<F> =
            unsafe { &*backend_user_data.cast::<BackendRegistration<F>>() };

        let inner_ptr =
            create_data_source::<F, D, DecoderByteStream>(decoder_stream, registration)?;

        backend.write(inner_ptr.cast::<sys::ma_data_source>());

        Ok::<_, MaudioError>(())
    }));

    match result {
        Ok(Ok(_)) => sys::ma_result_MA_SUCCESS,
        Ok(Err(e)) => e.ma_result.0,
        _ => sys::ma_result_MA_ERROR,
    }
}

unsafe extern "C" fn decoder_on_uninit<F: PcmFormat, D: DecodingBackend<Format = F>>(
    _user_data: *mut core::ffi::c_void,
    backend: *mut sys::ma_data_source,
    _alloc: *const sys::ma_allocation_callbacks,
) {
    drop(Box::from_raw(backend.cast::<BackendDataSource<F, D>>()));
}

fn create_data_source<F: PcmFormat, D: DecodingBackend<Format = F>, R: Read + Seek>(
    decoder_stream: R,
    registration: &BackendRegistration<F>,
) -> MaResult<*mut BackendDataSource<F, D>> {
    let decoder = D::init_decoder(decoder_stream)?;

    let mut builder = DataSourceBuilder::new(registration.channels, registration.sample_rate);
    let vtable = data_source_vtable::<F, D::Decoder>(&builder);
    builder.inner.vtable = vtable;

    let src_ctx = SourceContext {
        data_format: DataFormat {
            format: registration.format,
            channels: registration.channels,
            sample_rate: registration.sample_rate,
            channel_map: None,
        },
        cursor: 0,
        looping: false,
    };

    let mut inner: Box<BackendDataSource<F, D>> = Box::new(BackendDataSource {
        base: unsafe { MaybeUninit::zeroed().assume_init() },
        context: src_ctx,
        decoder,
        vtable,
        _format: PhantomData,
    });

    let base_ptr = core::ptr::addr_of_mut!(inner.base);

    data_source_ffi::ma_data_source_init(&builder, base_ptr.cast())?;

    let inner_ptr = Box::into_raw(inner);

    debug_assert_eq!(
        unsafe { core::ptr::addr_of_mut!((*inner_ptr).base) }.cast::<u8>(),
        inner_ptr.cast::<u8>(),
    );
    Ok(inner_ptr)
}
