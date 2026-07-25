use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{channels::Channel, formats::Format, sample_rate::SampleRate},
    data_source::{
        data_source_ffi, data_source_vtable::data_source_vtable, pcm_source::PcmSource, DataFormat,
        DataSource, DataSourceInner, SourceContext,
    },
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, MaResult,
};

pub struct DataSourceBuilder {
    pub(crate) inner: sys::ma_data_source_config,
    sample_rate: SampleRate,
    channels: u32,
    channel_map: Vec<Channel>,
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
            channel_map: Vec::new(),
        }
    }

    pub fn channel_map(&mut self, map: Vec<Channel>) -> &mut Self {
        self.channel_map = map;
        self
    }

    fn data_format(&mut self, format: Format) -> DataFormat {
        DataFormat {
            format,
            channels: self.channels,
            sample_rate: self.sample_rate,
            channel_map: Some(self.channel_map.clone()),
        }
    }

    fn build<F: PcmFormat, P: PcmSource<F>>(
        &mut self,
        source: P,
        context: SourceContext,
        vtable: *const sys::ma_data_source_vtable,
    ) -> MaResult<DataSource<F, P>> {
        self.inner.vtable = vtable;

        let mut inner = Box::new(DataSourceInner {
            // We must cast and access fields of this struct later.
            // Ensure base has a stable address before passing it to ma_data_source_init
            inner: unsafe { MaybeUninit::zeroed().assume_init() },
            context,
            source,
            vtable,
            _format: PhantomData,
        });

        let base_ptr = core::ptr::addr_of_mut!(inner.inner);

        data_source_ffi::ma_data_source_init(self, base_ptr.cast())?;

        let inner_ptr = Box::into_raw(inner);

        debug_assert_eq!(
            unsafe { core::ptr::addr_of_mut!((*inner_ptr).inner) }.cast::<u8>(),
            inner_ptr.cast::<u8>(),
        );

        Ok(DataSource { inner: inner_ptr })
    }

    pub fn build_u8<P: PcmSource<u8>>(&mut self, source: P) -> MaResult<DataSource<u8, P>> {
        let data_format = self.data_format(Format::U8);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<u8, P>();
        self.build::<u8, P>(source, context, vtable)
    }

    pub fn build_i16<P: PcmSource<i16>>(&mut self, source: P) -> MaResult<DataSource<i16, P>> {
        let data_format = self.data_format(Format::S16);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<i16, P>();
        self.build::<i16, P>(source, context, vtable)
    }

    pub fn build_i32<P: PcmSource<i32>>(&mut self, source: P) -> MaResult<DataSource<i32, P>> {
        let data_format = self.data_format(Format::S32);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<i32, P>();
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

        let vtable = data_source_vtable::<S24Packed, P>();
        self.build::<S24Packed, P>(source, context, vtable)
    }

    pub fn build_f32<P: PcmSource<f32>>(&mut self, source: P) -> MaResult<DataSource<f32, P>> {
        let data_format = self.data_format(Format::F32);
        let context = SourceContext {
            data_format,
            cursor: 0,
            looping: false,
        };

        let vtable = data_source_vtable::<f32, P>();
        self.build::<f32, P>(source, context, vtable)
    }
}
