use crate::{
    context::ContextBuilder,
    device::{device_info::DeviceInfo, device_type::DeviceType},
    logging::LogRef,
    ErrorKinds, MaResult, MaudioError,
};

pub trait CustomBackend {
    type Context;

    fn init_context(config: ContextBuilder, log: Option<LogRef>) -> MaResult<Self::Context>;

    fn enumerate_devices<F>(
        _context: &mut Self::Context,
        mut _f: F,
        _log: Option<LogRef>,
    ) -> MaResult<()>
    where
        F: FnMut(DeviceType, &DeviceInfo) -> bool,
    {
        Err(MaudioError::new_ma_error(ErrorKinds::NotImplemented))
    }
}
