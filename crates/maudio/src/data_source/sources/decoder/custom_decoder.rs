use std::{marker::PhantomData, mem::MaybeUninit, path::Path, sync::Arc};

use crate::{
    audio::{channels::Channel, formats::Format, sample_rate::SampleRate},
    data_source::{
        data_source_ffi, private_data_source,
        sources::decoder::{
            decoder_ffi, decoder_read_proc, decoder_seek_proc, decoder_vtable::decoder_vtable,
            decoding_backend::DecodingBackend, encoder_user_data_drop, private_decoder,
            AsDecoderPtr, Borrowed, Cb, DecoderOps, DecoderUserData, DecoderUserDataDestructor, Fs,
            Owned, SeekRead,
        },
        AsSourcePtr, SourceContext,
    },
    device::device_builder::Unknown,
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
};

use maudio_sys::ffi as sys;

pub struct CustomDecoder<F: PcmFormat, S> {
    inner: *mut sys::ma_decoder,
    channels: u32,
    #[allow(unused)]
    sample_rate: SampleRate,
    format: Format,
    user_data: Option<DecoderUserDataDestructor>,
    backend_reg: *mut BackendRegistration<F>,
    _source_data: S,                                                 // keep alive
    _decoder_vtables: Box<[*const sys::ma_decoding_backend_vtable]>, // keep alive
    _sample_format: PhantomData<F>,
}

unsafe impl<F: PcmFormat, S> Send for CustomDecoder<F, S> {}

/// `backend_user_data` inside the onInit in ma_decoding_backend_vtable
///
/// This object contains any data / config that can be used inside the init function
/// to create the data source and (potentially) as config for the `DecodingBackend` trait
///
/// The information in this object will be available to all the vtables (backends)
pub(crate) struct BackendRegistration<F>
where
    F: PcmFormat,
{
    pub(crate) channels: u32,
    pub(crate) sample_rate: SampleRate,
    pub(crate) format: Format,
    _format: PhantomData<F>,
}

/// The Data Source create inside the onInit
#[repr(C)]
pub(crate) struct BackendDataSource<F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F>,
{
    pub(crate) base: sys::ma_data_source_base,
    pub(crate) context: SourceContext,
    pub(crate) decoder: D::Decoder,
    pub(crate) vtable: *const sys::ma_data_source_vtable,
    pub(crate) _format: PhantomData<F>,
}

impl<F, D> AsRawRef for BackendDataSource<F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F>,
{
    type Raw = sys::ma_data_source_base;

    fn as_raw(&self) -> &Self::Raw {
        &self.base
    }
}

impl<F, D> Drop for BackendDataSource<F, D>
where
    F: PcmFormat,
    D: DecodingBackend<Format = F>,
{
    fn drop(&mut self) {
        drop(unsafe { Box::from_raw(self.vtable as *mut sys::ma_data_source_vtable) });
        data_source_ffi::ma_data_source_uninit(self.as_raw_ptr() as *mut _);
    }
}

impl<F: PcmFormat, S> Binding for CustomDecoder<F, S> {
    type Raw = *mut sys::ma_decoder;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat, S> AsDecoderPtr for CustomDecoder<F, S> {
    #[doc(hidden)]
    type __PtrProvider = private_decoder::CustomDecoderProvider;

    fn format(&self) -> Format {
        self.format
    }

    fn channels(&self) -> u32 {
        self.channels
    }
}

// Allows Decoder to pass as a DataSource
#[doc(hidden)]
impl<F: PcmFormat, S> AsSourcePtr for CustomDecoder<F, S> {
    type Format = F;
    type __PtrProvider = private_data_source::CustomDecoderProvider;
}

impl<F: PcmFormat, S> DecoderOps for CustomDecoder<F, S> {
    type Source = S;
}

impl<F: PcmFormat, S> CustomDecoder<F, S> {
    #[inline]
    fn new(
        inner: *mut sys::ma_decoder,
        config: &CustomDecoderBuilder<F>,
        format: Format,
        vtables: Box<[*const sys::ma_decoding_backend_vtable]>,
        reg: *mut BackendRegistration<F>,
        source_data: S,
    ) -> Self {
        Self {
            inner,
            channels: config.channels,
            sample_rate: config.sample_rate,
            format,
            user_data: None,
            backend_reg: reg,
            _source_data: source_data,
            _sample_format: PhantomData,
            _decoder_vtables: vtables,
        }
    }

    fn init_file(
        path: &Path,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Fs>> {
        let vtables = config.set_backend_vtables()?;
        let reg = config.set_backend_registration();

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        CustomDecoder::<F, S>::init_from_file_internal(path, config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Fs,
        ))
    }

    fn init_from_file_internal(
        path: &Path,
        config: &CustomDecoderBuilder<F>,
        decoder: *mut sys::ma_decoder,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            use crate::engine::cstring_from_path;

            let path = cstring_from_path(path)?;
            decoder_ffi::ma_decoder_init_file(path, config.as_raw_ptr(), decoder)
        }

        #[cfg(windows)]
        {
            use crate::engine::wide_null_terminated;

            let path = wide_null_terminated(path);

            decoder_ffi::ma_decoder_init_file_w(&path, config.as_raw_ptr(), decoder)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }
    fn init_from_reader<R: SeekRead>(
        reader: R,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Cb>> {
        let vtables = config.set_backend_vtables()?;
        let reg = config.set_backend_registration();
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        let user_data = Box::new(DecoderUserData { reader });
        let user_data_ptr = Box::into_raw(user_data) as *mut _;

        if let Err(e) = decoder_ffi::ma_decoder_init(
            Some(decoder_read_proc::<R>),
            Some(decoder_seek_proc::<R>),
            user_data_ptr,
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        ) {
            drop(unsafe { Box::from_raw(user_data_ptr as *mut DecoderUserData<R>) });
            return Err(e);
        }

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        let mut decoder = CustomDecoder::new(inner, config, config.format, vtables, reg, Cb);
        decoder.user_data = Some((user_data_ptr, encoder_user_data_drop::<R>));

        Ok(decoder)
    }

    fn init_from_memory<'a>(
        data: &'a [u8],
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Borrowed<'a>>> {
        let vtables = config.set_backend_vtables()?;
        let reg = config.set_backend_registration();

        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data.as_ptr() as *const _,
            data.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Borrowed(data),
        ))
    }

    fn init_copy<D: Into<Arc<[u8]>>>(
        data: D,
        config: &mut CustomDecoderBuilder<F>,
    ) -> MaResult<CustomDecoder<F, Owned>> {
        let vtables = config.set_backend_vtables()?;
        let reg = config.set_backend_registration();

        let data_arc = data.into();
        let mut mem: Box<std::mem::MaybeUninit<sys::ma_decoder>> = Box::new(MaybeUninit::uninit());

        decoder_ffi::ma_decoder_init_memory(
            data_arc.as_ptr() as *const _,
            data_arc.len(),
            config.as_raw_ptr(),
            mem.as_mut_ptr(),
        )?;

        let inner: *mut sys::ma_decoder = Box::into_raw(mem) as *mut sys::ma_decoder;
        Ok(CustomDecoder::new(
            inner,
            config,
            config.format,
            vtables,
            reg,
            Owned(data_arc),
        ))
    }
}

pub struct CustomDecoderBuilder<F = Unknown> {
    inner: sys::ma_decoder_config,
    vtables: Vec<*const sys::ma_decoding_backend_vtable>,
    sample_rate: SampleRate,
    channels: u32,
    format: Format,
    #[allow(unused)]
    channel_map: Option<Vec<Channel>>,
    // user_data: Option<U>,
    _format: PhantomData<F>,
}

impl<F: PcmFormat> AsRawRef for CustomDecoderBuilder<F> {
    type Raw = sys::ma_decoder_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl CustomDecoderBuilder<Unknown> {
    #[inline]
    fn new_inner(
        out_channels: u32,
        out_sample_rate: SampleRate,
        format: Format,
    ) -> sys::ma_decoder_config {
        unsafe { sys::ma_decoder_config_init(format.into(), out_channels, out_sample_rate.into()) }
    }

    pub fn new_u8(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<u8> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::U8);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::U8,
            channel_map: None,
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_i16(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<i16> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S16);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S16,
            channel_map: None,
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_i32(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<i32> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S32);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S32,
            channel_map: None,
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_s24_packed(
        out_channels: u32,
        out_sample_rate: SampleRate,
    ) -> CustomDecoderBuilder<S24Packed> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::S24Packed);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::S24Packed,
            channel_map: None,
            // user_data: None,
            _format: PhantomData,
        }
    }

    pub fn new_f32(out_channels: u32, out_sample_rate: SampleRate) -> CustomDecoderBuilder<f32> {
        let inner = Self::new_inner(out_channels, out_sample_rate, Format::F32);
        CustomDecoderBuilder {
            inner,
            vtables: Vec::new(),
            sample_rate: out_sample_rate,
            channels: out_channels,
            format: Format::F32,
            channel_map: None,
            // user_data: None,
            _format: PhantomData,
        }
    }
}

impl<F: PcmFormat> CustomDecoderBuilder<F> {
    fn set_backend_registration(&mut self) -> *mut BackendRegistration<F> {
        let registration: BackendRegistration<F> = BackendRegistration {
            channels: self.channels,
            sample_rate: self.sample_rate,
            format: self.format,
            _format: PhantomData,
        };
        let ptr = Box::into_raw(Box::new(registration));
        self.inner.pCustomBackendUserData = ptr.cast();
        ptr
    }

    fn set_backend_vtables(&mut self) -> MaResult<Box<[*const sys::ma_decoding_backend_vtable]>> {
        if self.vtables.is_empty() {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "At least one backend mut be provided.",
            )));
        }
        self.inner.customBackendCount = self.vtables.len() as u32;

        // Each backend is converted to a vtable and stored in a vec
        // Drain that vec and store the vtables into a Box with a stable address
        let mut vtables: Box<[*const sys::ma_decoding_backend_vtable]> =
            self.vtables.drain(..).collect();
        self.inner.ppCustomBackendVTables =
            vtables.as_mut_ptr() as *mut *mut sys::ma_decoding_backend_vtable;

        Ok(vtables)
    }

    pub fn backend<B: DecodingBackend<Format = F>>(&mut self) -> &mut Self {
        let vtable = decoder_vtable::<F, B>();
        self.vtables.push(vtable);
        self
    }

    /// Creates a decoder from borrowed in-memory audio data.
    ///
    /// This uses `ma_decoder_init_memory`.
    ///
    /// The input bytes are borrowed for the lifetime of the returned decoder,
    /// so the data must remain valid for as long as the decoder exists.
    ///
    /// This is the most direct in-memory constructor when you already have the
    /// full encoded audio data available and can keep it alive externally.
    pub fn from_memory<'a>(&mut self, data: &'a [u8]) -> MaResult<CustomDecoder<F, Borrowed<'a>>> {
        CustomDecoder::<F, Borrowed<'a>>::init_from_memory(data, self)
    }

    /// Creates a decoder from owned in-memory audio data.
    ///
    /// This is the same as from_memory, but stores an owned copy of the
    /// encoded data inside the returned decoder.
    ///
    /// Use this when you want the decoder to own backing memory
    /// instead of borrowing it from the caller.
    pub fn copy_memory<D: Into<Arc<[u8]>>>(
        &mut self,
        data: D,
    ) -> MaResult<CustomDecoder<F, Owned>> {
        CustomDecoder::<F, Owned>::init_copy(data, self)
    }

    /// Creates a decoder from a file path.
    ///
    /// The file is opened and managed through miniaudio's file-based decoding
    /// path rather than by storing the file contents in Rust memory first.
    ///
    /// This is usually the most convenient option when decoding from a normal
    /// file on disk.
    pub fn from_file(&mut self, path: &Path) -> MaResult<CustomDecoder<F, Fs>> {
        CustomDecoder::<F, Fs>::init_file(path, self)
    }

    /// Creates a decoder from a custom Rust reader.
    ///
    /// The reader must implement [`SeekRead`], meaning it supports both
    /// [`std::io::Read`] and [`std::io::Seek`]. This makes it suitable for
    /// file-like sources such as:
    ///
    /// - [`std::fs::File`]
    /// - [`std::io::Cursor`]
    /// - buffered wrappers around seekable readers
    ///
    /// This constructor is intended for custom, seekable data sources.
    /// It is best suited to sources that behave like regular files or in-memory
    /// byte buffers.
    ///
    /// # Notes
    /// The reader is owned by the decoder and accessed through internal
    /// callbacks required by miniaudio.
    ///
    /// This constructor is not ideal for temporary "data not available yet"
    /// situations. The supplied reader should behave like a normal seekable
    /// byte source.
    ///
    /// If the source behaves like a stream and may temporarily provide fewer
    /// bytes than requested, this will be treated as EOF,
    /// and decoding will stop instead of waiting for more data.
    pub fn from_reader<R: SeekRead>(&mut self, reader: R) -> MaResult<CustomDecoder<F, Cb>> {
        CustomDecoder::<F, Cb>::init_from_reader(reader, self)
    }
}

impl<F: PcmFormat, S> Drop for CustomDecoder<F, S> {
    fn drop(&mut self) {
        let _ = decoder_ffi::ma_decoder_uninit(self);
        for vtable in self._decoder_vtables.iter() {
            drop(unsafe { Box::from_raw(*vtable as *mut sys::ma_decoding_backend_vtable) });
        }
        if let Some((ptr, destructor)) = self.user_data {
            destructor(ptr);
        }
        drop(unsafe { Box::from_raw(self.backend_reg) });
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(test)]
mod test {
    use crate::test_assets::{
        temp_file::{unique_tmp_path, TempFileGuard},
        wav_i16_le,
    };
    use crate::{
        audio::sample_rate::SampleRate,
        data_source::{
            pcm_source::PcmSource,
            sources::decoder::{
                decoding_backend::DecodingBackend, Cb, Decoder, DecoderBuilder, DecoderOps,
            },
        },
        MaResult,
    };

    use super::*;

    fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
        let mut samples = Vec::with_capacity(frames);
        for i in 0..frames {
            samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
        }
        wav_i16_le(1, SampleRate::Sr48000, &samples)
    }

    struct TestCbDecoder(Decoder<f32, Cb>);

    impl DecodingBackend for TestCbDecoder {
        type Format = f32;

        type Decoder = TestCbDecoder;

        fn init_decoder<R: std::io::prelude::Read + std::io::prelude::Seek>(
            stream: R,
        ) -> MaResult<Self::Decoder> {
            let decoder = DecoderBuilder::new_f32(1, SampleRate::Sr48000).from_reader(stream)?;
            Ok(TestCbDecoder(decoder))
        }
    }

    impl PcmSource<f32> for TestCbDecoder {
        fn fill_pcm_frames(
            &mut self,
            out: &mut [f32],
            ctx: &mut crate::data_source::SourceContext,
        ) -> MaResult<usize> {
            let frames = self.0.read_pcm_frames_into(out).unwrap_or(0);
            ctx.cursor += frames as u64;

            Ok(frames)
        }

        fn seek_to_pcm_frame(
            &mut self,
            frame_index: u64,
            ctx: &mut crate::data_source::SourceContext,
        ) -> MaResult<()> {
            if DecoderOps::seek_to_pcm_frame(&mut self.0, frame_index).is_ok() {
                ctx.cursor = frame_index
            }
            Ok(())
        }

        fn cursor_in_pcm_frames(&self, ctx: &crate::data_source::SourceContext) -> Option<u64> {
            Some(ctx.cursor)
        }

        fn length_in_pcm_frames(&self, _ctx: &crate::data_source::SourceContext) -> Option<u64> {
            self.0.length_pcm().ok()
        }

        fn set_looping(
            &self,
            _looping: bool,
            _ctx: &mut crate::data_source::SourceContext,
        ) -> MaResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_custom_decoder_from_memory_f32_read_seek_cursor_length_available() {
        let frames_total: usize = 64;
        let wav = tiny_test_wav_mono(frames_total);

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec = builder.backend::<TestCbDecoder>().copy_memory(wav).unwrap();

        let len = dec.length_pcm().unwrap();
        assert_eq!(len as usize, frames_total);

        let cursor0 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor0, 0);

        let avail0 = dec.available_frames().unwrap();
        assert_eq!(avail0 as usize, frames_total);

        let df = dec.data_format().unwrap();
        assert_eq!(df.channels, 1);
        assert_eq!(df.sample_rate, SampleRate::Sr48000);
        assert_eq!(df.format, Format::F32);

        let buf = dec.read_pcm_frames(10).unwrap();
        let read = buf.frames();
        assert_eq!(read, 10);
        assert_eq!(buf.len(), 10);

        let cursor1 = dec.cursor_pcm().unwrap();
        assert_eq!(cursor1, 10);

        let avail1 = dec.available_frames().unwrap();
        assert_eq!(avail1 as usize, frames_total - 10);

        dec.seek_to_pcm_frame(0).unwrap();
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let buf2 = dec.read_pcm_frames(7).unwrap();
        let read2 = buf2.frames();
        assert_eq!(read2, 7);
        assert_eq!(dec.cursor_pcm().unwrap(), 7);
    }

    #[test]
    fn test_custom_decoder_ref_from_memory_decodes() {
        let frames_total: usize = 32;
        let wav = tiny_test_wav_mono(frames_total);

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec_ref = builder
            .backend::<TestCbDecoder>()
            .from_memory(&wav)
            .unwrap();

        let buf = dec_ref.read_pcm_frames(12).unwrap();
        let read = buf.frames();
        assert_eq!(read, 12);
        assert_eq!(buf.len(), 12);
    }

    #[test]
    fn test_custom_decoder_from_file_reads_and_reports_length() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut builder = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000);

        let mut dec = builder
            .backend::<TestCbDecoder>()
            .from_file(guard.path())
            .unwrap();

        assert_eq!(dec.length_pcm().unwrap() as usize, frames_total);
        assert_eq!(dec.cursor_pcm().unwrap(), 0);

        let buf = dec.read_pcm_frames(1000).unwrap();
        assert_eq!(buf.frames(), frames_total);
    }

    #[test]
    fn test_custom_decoder_read_f32_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_memory(&wav)
            .unwrap();

        let avail = dec.available_frames().unwrap();
        println!("avail: {avail}");

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_custom_decoder_read_f32_copy_memory() {
        let frames_total: usize = 16;
        let wav = tiny_test_wav_mono(frames_total);
        let wav: Arc<[u8]> = wav.into();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .copy_memory(wav.clone())
            .unwrap();

        let b = dec.read_pcm_frames(5).unwrap();

        assert_eq!(b.frames(), 5);
    }

    #[test]
    fn test_custom_decoder_read_f32_path() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_file(guard.path())
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }

    #[test]
    fn test_custom_decoder_read_f32_file() {
        let frames_total: usize = 40;
        let wav = tiny_test_wav_mono(frames_total);

        let guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(guard.path(), &wav).unwrap();
        let file = std::fs::File::open(guard.path()).unwrap();

        let mut dec = CustomDecoderBuilder::new_f32(1, SampleRate::Sr48000)
            .backend::<TestCbDecoder>()
            .from_reader(file)
            .unwrap();

        let b = dec.read_pcm_frames(40).unwrap();

        assert_eq!(b.frames(), 40);
    }
}
