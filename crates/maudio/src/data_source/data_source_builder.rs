use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{channels::Channel, formats::Format, sample_rate::SampleRate},
    data_source::{
        data_source_ffi, data_source_vtable::data_source_vtable, pcm_source::PcmSource, DataFormat,
        DataSource, SourceContext,
    },
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, MaResult,
};

pub struct DataSourceBuilder {
    inner: sys::ma_data_source_config,
    sample_rate: SampleRate,
    channels: u32,
    channel_map: Option<Vec<Channel>>,
    pub(crate) no_looping: bool,
    pub(crate) no_length: bool,
    pub(crate) no_seek: bool,
    pub(crate) no_cursor: bool,
}

impl AsRawRef for DataSourceBuilder {
    type Raw = sys::ma_data_source_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}

impl DataSourceBuilder {
    pub fn new(channels: u32, sample_rate: SampleRate) -> Self {
        let inner = unsafe { sys::ma_data_source_config_init() };
        Self {
            inner,
            sample_rate,
            channels,
            channel_map: None,
            no_looping: false,
            no_length: false,
            no_seek: false,
            no_cursor: false,
        }
    }

    pub fn channel_map(&mut self, map: Vec<Channel>) -> &mut Self {
        self.channel_map = Some(map);
        self
    }

    /// This does not enable or disable looping. It disables the ability to enable or disable looping on the resulting data source.
    ///
    /// Looping support is enabled by default.
    ///
    /// When disabled, the following will return `MA_NOT_IMPLEMENTED`:
    /// - `DataSource::set_looping`
    /// - `DataSource::looping`
    ///
    /// If looping support is enabled and looping is turned on, the actual looping
    /// behavior must be implemented by [`PcmSource::fill_pcm_frames`]. The
    /// [`DataSource`] only stores and exposes the looping flag; it does not
    /// automatically rewind or repeat the source.
    ///
    /// Source implementations can inspect the looping flag through
    /// [`SourceContext`] when filling PCM frames.
    pub fn no_looping(&mut self, no: bool) -> &mut Self {
        self.no_looping = no;
        self
    }

    /// False by default
    ///
    /// This disables maudio methods that access length. Specifically:
    /// - `DataSource::length_in_pcm_frames`
    /// - `DataSource::length_in_seconds`
    ///
    pub fn no_length(&mut self, no: bool) -> &mut Self {
        self.no_length = no;
        self
    }

    /// Enables or disables `DataSource`-managed seeking.
    ///
    /// Seeking is enabled by default.
    ///
    /// When seeking is enabled, `DataSource` owns and manages the cursor.
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
    pub fn no_seek(&mut self, no: bool) -> &mut Self {
        self.no_seek = no;
        self
    }

    /// False by default
    pub fn no_cursor(&mut self, no: bool) -> &mut Self {
        self.no_cursor = no;
        self
    }

    fn data_format(&mut self, format: Format) -> DataFormat {
        DataFormat {
            format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            channel_map: self.channel_map.take(),
        }
    }

    fn build<F: PcmFormat, P: PcmSource<F>>(
        &mut self,
        source: P,
        context: SourceContext,
        vtable: *const sys::ma_data_source_vtable,
    ) -> MaResult<DataSource<F, P>> {
        self.inner.vtable = vtable;
        let mut inner: MaybeUninit<sys::ma_data_source_base> = MaybeUninit::uninit();

        data_source_ffi::ma_data_source_init(self, inner.as_mut_ptr() as *mut _)?;

        Ok(DataSource {
            inner: unsafe { inner.assume_init() },
            source,
            context,
            vtable,
            _format: PhantomData,
        })
    }

    pub fn build_u8<P: PcmSource<u8>>(&mut self, source: P) -> MaResult<DataSource<u8, P>> {
        let data_format = self.data_format(Format::U8);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<u8, P>(self);
        self.build::<u8, P>(source, context, vtable)
    }

    pub fn build_i16<P: PcmSource<i16>>(&mut self, source: P) -> MaResult<DataSource<i16, P>> {
        let data_format = self.data_format(Format::S16);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<i16, P>(self);
        self.build::<i16, P>(source, context, vtable)
    }

    pub fn build_i32<P: PcmSource<i32>>(&mut self, source: P) -> MaResult<DataSource<i32, P>> {
        let data_format = self.data_format(Format::S32);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<i32, P>(self);
        self.build::<i32, P>(source, context, vtable)
    }

    pub fn build_s24_packed<P: PcmSource<S24Packed>>(
        &mut self,
        source: P,
    ) -> MaResult<DataSource<S24Packed, P>> {
        let data_format = self.data_format(Format::S24Packed);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<S24Packed, P>(self);
        self.build::<S24Packed, P>(source, context, vtable)
    }

    pub fn build_f32<P: PcmSource<f32>>(&mut self, source: P) -> MaResult<DataSource<f32, P>> {
        let data_format = self.data_format(Format::F32);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<f32, P>(self);
        self.build::<f32, P>(source, context, vtable)
    }
}
