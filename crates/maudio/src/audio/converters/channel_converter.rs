use std::{marker::PhantomData, mem::MaybeUninit};

use maudio_sys::ffi as sys;

use crate::{
    audio::{
        channels::{Channel, ChannelMixMode, RawChannel},
        formats::Format,
    },
    pcm_frames::{PcmFormat, S24Packed},
    AsRawRef, Binding, MaResult,
};

pub struct ChannelConverter<F: PcmFormat> {
    inner: *mut sys::ma_channel_converter,
    channels_in: u32,
    channels_out: u32,
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for ChannelConverter<F> {}

impl<F: PcmFormat> Binding for ChannelConverter<F> {
    type Raw = *mut sys::ma_channel_converter;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

impl<F: PcmFormat> ChannelConverter<F> {
    pub fn process_pcm_frames_into(
        &mut self,
        input: &[F::StorageUnit],
        output: &mut [F::StorageUnit],
    ) -> MaResult<()> {
        channel_converter_ffi::ma_channel_converter_process_pcm_frames_into(self, input, output)
    }

    pub fn process_pcm_frames(
        &mut self,
        input: &[F::StorageUnit],
    ) -> MaResult<Vec<F::StorageUnit>> {
        channel_converter_ffi::ma_channel_converter_process_pcm_frames(self, input)
    }

    pub fn input_channel_map(&self) -> MaResult<Vec<Channel>> {
        channel_converter_ffi::ma_channel_converter_get_input_channel_map(self)
    }

    pub fn output_channel_map(&self) -> MaResult<Vec<Channel>> {
        channel_converter_ffi::ma_channel_converter_get_output_channel_map(self)
    }
}

// Private methods
impl<F: PcmFormat> ChannelConverter<F> {
    fn new_with_config(config: &ChannelConverterBuilder) -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_channel_converter>> = Box::new(MaybeUninit::uninit());

        channel_converter_ffi::ma_channel_converter_init(config, mem.as_mut_ptr())?;

        let inner: *mut sys::ma_channel_converter =
            Box::into_raw(mem) as *mut sys::ma_channel_converter;

        Ok(Self {
            inner,
            channels_in: config.channels_in,
            channels_out: config.channels_out,
            _format: PhantomData,
        })
    }
}

pub struct ChannelConverterBuilder {
    config: sys::ma_channel_converter_config,
    channels_in: u32,
    channels_out: u32,
    in_channel_map: Vec<RawChannel>,
    out_channel_map: Vec<RawChannel>,
}

impl AsRawRef for ChannelConverterBuilder {
    type Raw = sys::ma_channel_converter_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.config
    }
}

impl ChannelConverterBuilder {
    pub fn new(in_channel_count: u32, out_channel_count: u32) -> Self {
        let config = unsafe {
            sys::ma_channel_converter_config_init(
                Format::U8.into(),
                in_channel_count,
                std::ptr::null(),
                out_channel_count,
                std::ptr::null(),
                ChannelMixMode::Default.into(),
            )
        };
        Self {
            config,
            channels_in: in_channel_count,
            channels_out: out_channel_count,
            in_channel_map: Vec::new(),
            out_channel_map: Vec::new(),
        }
    }

    pub fn mix_mode(&mut self, mode: ChannelMixMode) -> &mut Self {
        self.config.mixingMode = mode.into();
        self
    }

    pub fn in_channel_map<I>(&mut self, channel_map: I) -> &mut Self
    where
        I: IntoIterator<Item = Channel>,
    {
        self.in_channel_map = channel_map.into_iter().map(RawChannel::from).collect();
        self.config.pChannelMapIn = self.in_channel_map.as_ptr() as *const _;
        self
    }

    pub fn out_channel_map<I>(&mut self, channel_map: I) -> &mut Self
    where
        I: IntoIterator<Item = Channel>,
    {
        self.out_channel_map = channel_map.into_iter().map(RawChannel::from).collect();
        self.config.pChannelMapOut = self.out_channel_map.as_ptr() as *const _;
        self
    }

    pub fn build_u8(&mut self) -> MaResult<ChannelConverter<u8>> {
        self.config.format = Format::U8.into();
        ChannelConverter::new_with_config(self)
    }

    pub fn build_i16(&mut self) -> MaResult<ChannelConverter<i16>> {
        self.config.format = Format::S16.into();
        ChannelConverter::new_with_config(self)
    }

    pub fn build_s24_packed(&mut self) -> MaResult<ChannelConverter<S24Packed>> {
        self.config.format = Format::S24Packed.into();
        ChannelConverter::new_with_config(self)
    }

    pub fn build_i32(&mut self) -> MaResult<ChannelConverter<i32>> {
        self.config.format = Format::S32.into();
        ChannelConverter::new_with_config(self)
    }

    pub fn build_f32(&mut self) -> MaResult<ChannelConverter<f32>> {
        self.config.format = Format::F32.into();
        ChannelConverter::new_with_config(self)
    }
}

mod channel_converter_ffi {
    use crate::{
        audio::{
            channels::{Channel, RawChannel},
            converters::channel_converter::{ChannelConverter, ChannelConverterBuilder},
        },
        pcm_frames::PcmFormat,
        AllocationCallbacks, AsRawRef, Binding, ErrorKinds, MaResult, MaudioError,
    };
    use maudio_sys::ffi as sys;

    pub fn ma_channel_converter_init(
        config: &ChannelConverterBuilder,
        converter: *mut sys::ma_channel_converter,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_channel_converter_init(
                config.as_raw_ptr(),
                AllocationCallbacks::cb_ptr(),
                converter,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_channel_converter_uninit<F: PcmFormat>(converter: &mut ChannelConverter<F>) {
        unsafe {
            sys::ma_channel_converter_uninit(converter.to_raw(), AllocationCallbacks::cb_ptr());
        }
    }

    pub fn ma_channel_converter_process_pcm_frames<F: PcmFormat>(
        converter: &mut ChannelConverter<F>,
        input: &[F::StorageUnit],
    ) -> MaResult<Vec<F::StorageUnit>> {
        if input.len() / converter.channels_in as usize == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Input length too small",
            )));
        }

        let frames = input.len() / (converter.channels_in as usize * F::VEC_STORE_UNITS_PER_FRAME);
        let samples_out = frames * converter.channels_out as usize * F::VEC_STORE_UNITS_PER_FRAME;
        let mut out = vec![F::STORE_SILENCE; samples_out];

        let res = unsafe {
            sys::ma_channel_converter_process_pcm_frames(
                converter.to_raw(),
                out.as_mut_ptr() as *mut _,
                input.as_ptr() as *mut _,
                frames as u64,
            )
        };
        MaudioError::check(res)?;

        Ok(out)
    }

    pub fn ma_channel_converter_process_pcm_frames_into<F: PcmFormat>(
        converter: &mut ChannelConverter<F>,
        input: &[F::StorageUnit],
        output: &mut [F::StorageUnit],
    ) -> MaResult<()> {
        if input.len() / converter.channels_in as usize == 0 {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Input length too small",
            )));
        }

        let frames = input.len() / (converter.channels_in as usize * F::VEC_STORE_UNITS_PER_FRAME);
        let samples_out = frames * converter.channels_out as usize * F::VEC_STORE_UNITS_PER_FRAME;
        if output.len() < samples_out {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Output length too small",
            )));
        }

        let res = unsafe {
            sys::ma_channel_converter_process_pcm_frames(
                converter.to_raw(),
                output.as_mut_ptr() as *mut _,
                input.as_ptr() as *mut _,
                frames as u64,
            )
        };
        MaudioError::check(res)
    }

    pub fn ma_channel_converter_get_input_channel_map<F: PcmFormat>(
        converter: &ChannelConverter<F>,
    ) -> MaResult<Vec<Channel>> {
        let mut raw_map = vec![RawChannel::from_raw(0); converter.channels_in as usize];

        let res = unsafe {
            sys::ma_channel_converter_get_input_channel_map(
                converter.to_raw(),
                raw_map.as_mut_ptr() as *mut _,
                converter.channels_in as usize,
            )
        };
        MaudioError::check(res)?;

        let mut map = vec![];
        for &ch in raw_map.iter() {
            map.push(Channel::try_from(ch)?);
        }
        Ok(map)
    }

    pub fn ma_channel_converter_get_output_channel_map<F: PcmFormat>(
        converter: &ChannelConverter<F>,
    ) -> MaResult<Vec<Channel>> {
        let mut raw_map = vec![RawChannel::from_raw(0); converter.channels_out as usize];

        let res = unsafe {
            sys::ma_channel_converter_get_output_channel_map(
                converter.to_raw(),
                raw_map.as_mut_ptr() as *mut _,
                converter.channels_in as usize,
            )
        };
        MaudioError::check(res)?;

        let mut map = vec![];
        for &ch in raw_map.iter() {
            map.push(Channel::try_from(ch)?);
        }
        Ok(map)
    }
}

impl<F: PcmFormat> Drop for ChannelConverter<F> {
    fn drop(&mut self) {
        channel_converter_ffi::ma_channel_converter_uninit(self);
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(test)]
mod test {
    use crate::audio::{
        channels::Channel, converters::channel_converter::ChannelConverterBuilder, formats::Format,
    };

    #[test]
    fn test_channel_converter_basic_init_u8() {
        let res = ChannelConverterBuilder::new(1, 2).build_u8();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let format = unsafe { &*conv.inner }.format;
        assert_eq!(format, Format::U8.into());
    }

    #[test]
    fn test_channel_converter_basic_init_i16() {
        let res = ChannelConverterBuilder::new(1, 2).build_i16();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let format = unsafe { &*conv.inner }.format;
        assert_eq!(format, Format::S16.into());
    }

    #[test]
    fn test_channel_converter_basic_init_s24_packed() {
        let res = ChannelConverterBuilder::new(1, 2).build_s24_packed();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let format = unsafe { &*conv.inner }.format;
        assert_eq!(format, Format::S24Packed.into());
    }

    #[test]
    fn test_channel_converter_basic_init_i32() {
        let res = ChannelConverterBuilder::new(1, 2).build_i32();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let format = unsafe { &*conv.inner }.format;
        assert_eq!(format, Format::S32.into());
    }

    #[test]
    fn test_channel_converter_basic_init_f32() {
        let res = ChannelConverterBuilder::new(1, 2).build_f32();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let format = unsafe { &*conv.inner }.format;
        assert_eq!(format, Format::F32.into());
    }

    #[test]
    fn test_channel_converter_zero_in_ch() {
        let res = ChannelConverterBuilder::new(0, 2).build_f32();
        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_zero_out_ch() {
        let res = ChannelConverterBuilder::new(1, 0).build_f32();
        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_in_map_basic() {
        let in_map = [Channel::SideLeft, Channel::SideRight];
        let conv = ChannelConverterBuilder::new(2, 4)
            .in_channel_map(in_map)
            .build_f32()
            .unwrap();
        let map_check = conv.input_channel_map().unwrap();
        assert_eq!(map_check, in_map);
    }

    #[test]
    fn test_channel_converter_out_map_basic() {
        let in_map = [Channel::SideLeft, Channel::SideRight];
        let conv = ChannelConverterBuilder::new(1, 2)
            .out_channel_map(in_map)
            .build_f32()
            .unwrap();
        let map_check = conv.output_channel_map().unwrap();
        assert_eq!(map_check, in_map);
    }

    #[test]
    fn test_channel_converter_in_map_different_chanels() {
        let in_map = [Channel::SideLeft, Channel::SideRight];
        let res = ChannelConverterBuilder::new(1, 2)
            .in_channel_map(in_map)
            .build_f32();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let in_ch = unsafe { &*conv.inner }.channelsIn;
        assert_eq!(in_ch, 1);
        let map = conv.input_channel_map().unwrap();
        assert_eq!(map, [Channel::SideLeft]);
    }

    #[test]
    fn test_channel_converter_out_map_different_chanels() {
        let in_map = [
            Channel::SideLeft,
            Channel::SideRight,
            Channel::TopBackCenter,
            Channel::TopFrontCenter,
        ];
        let res = ChannelConverterBuilder::new(1, 2)
            .out_channel_map(in_map)
            .build_f32();
        assert!(res.is_ok());
        let conv = res.unwrap();
        let in_ch = unsafe { &*conv.inner }.channelsOut;
        assert_eq!(in_ch, 2);
        let map = conv.output_channel_map().unwrap();
        assert_eq!(map, [Channel::SideLeft, Channel::SideRight]);
    }

    #[test]
    fn test_channel_converter_mono_to_stereo_basic() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_f32().unwrap();

        let input = [0.25, -0.5, 1.0];

        let output = converter.process_pcm_frames(&input).unwrap();

        assert_eq!(
            output,
            [
                0.25, 0.25, // frame 1
                -0.5, -0.5, // frame 2
                1.0, 1.0, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_mono_to_stereo_into() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_f32().unwrap();

        let input = [0.25, -0.5, 1.0];

        let mut output = vec![0.0; 6];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                0.25, 0.25, // frame 1
                -0.5, -0.5, // frame 2
                1.0, 1.0, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_mono_to_stereo_under_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_f32().unwrap();

        let input = [0.25, -0.5, 1.0];

        let mut output = vec![0.0; 5];
        let res = converter.process_pcm_frames_into(&input, &mut output);
        assert!(res.is_err())
    }

    #[test]
    fn test_channel_converter_mono_to_stereo_over_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_f32().unwrap();

        let input = [0.25, -0.5, 1.0];

        let mut output = vec![0.0; 7];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                0.25, 0.25, // frame 1
                -0.5, -0.5, // frame 2
                1.0, 1.0, // frame 3
                0.0  // extra
            ]
        );
    }

    #[test]
    fn test_channel_converter_u8_mono_to_stereo_basic() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_u8().unwrap();

        let input = [1, 2, 3];

        let output = converter.process_pcm_frames(&input).unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_u8_mono_to_stereo_into() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_u8().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 6];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_u8_mono_to_stereo_under_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_u8().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 5];
        let res = converter.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_u8_mono_to_stereo_over_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_u8().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 7];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
                0, // extra
            ]
        );
    }

    #[test]
    fn test_channel_converter_i16_mono_to_stereo_basic() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i16().unwrap();

        let input = [1, 2, 3];

        let output = converter.process_pcm_frames(&input).unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_i16_mono_to_stereo_into() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i16().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 6];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_i16_mono_to_stereo_under_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i16().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 5];
        let res = converter.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_i16_mono_to_stereo_over_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i16().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 7];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
                0, // extra
            ]
        );
    }

    #[test]
    fn test_channel_converter_s24_packed_mono_to_stereo_basic() {
        let mut converter = ChannelConverterBuilder::new(1, 2)
            .build_s24_packed()
            .unwrap();

        let input = [1, 2, 3];

        let output = converter.process_pcm_frames(&input).unwrap();

        assert_eq!(
            output,
            [
                1, 2, 3, // frame 1, left channel
                1, 2, 3, // frame 1, right channel
            ]
        );
    }

    #[test]
    fn test_channel_converter_s24_packed_mono_to_stereo_into() {
        let mut converter = ChannelConverterBuilder::new(1, 2)
            .build_s24_packed()
            .unwrap();

        let input = [1, 2, 3, 4, 5, 6];

        let mut output = vec![0; 12];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 2, 3, // first sample, left channel
                1, 2, 3, // first sample, right channel
                4, 5, 6, // second sample, left channel
                4, 5, 6, // second sample, right channel
            ]
        );
    }

    #[test]
    fn test_channel_converter_s24_packed_mono_to_stereo_under_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2)
            .build_s24_packed()
            .unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 5];
        let res = converter.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_s24_packed_mono_to_stereo_over_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2)
            .build_s24_packed()
            .unwrap();

        let input = [1, 0, 0, 2, 0, 0];

        let mut output = vec![0; 13];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 0, 0, // frame 1, left channel
                1, 0, 0, // frame 1, right channel
                2, 0, 0, // frame 2, left channel,
                2, 0, 0, // frame 2, right channel,
                0, // extra
            ]
        );
    }

    #[test]
    fn test_channel_converter_i32_mono_to_stereo_basic() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i32().unwrap();

        let input = [1, 2, 3];

        let output = converter.process_pcm_frames(&input).unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_i32_mono_to_stereo_into() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i32().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 6];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
            ]
        );
    }

    #[test]
    fn test_channel_converter_i32_mono_to_stereo_under_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i32().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 5];
        let res = converter.process_pcm_frames_into(&input, &mut output);

        assert!(res.is_err());
    }

    #[test]
    fn test_channel_converter_i32_mono_to_stereo_over_size() {
        let mut converter = ChannelConverterBuilder::new(1, 2).build_i32().unwrap();

        let input = [1, 2, 3];

        let mut output = vec![0; 7];
        converter
            .process_pcm_frames_into(&input, &mut output)
            .unwrap();

        assert_eq!(
            output,
            [
                1, 1, // frame 1
                2, 2, // frame 2
                3, 3, // frame 3
                0, // extra
            ]
        );
    }
}
