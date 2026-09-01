//! Logging support for miniaudio objects.
//!
//! A [`Log`] can be created independently and shared with engines, contexts or devices.
//! Logs associated with an existing engine, context or device can be accessed through
//! [`LogRef`].
//!
//! Use [`LogOps::print_level`] for simple printing to standard error, or
//! [`LogOps::register_log`] to process log messages with a custom callback.
use std::sync::{Mutex, OnceLock};
use std::{ffi::c_void, fmt::Display, mem::MaybeUninit, panic::AssertUnwindSafe, sync::Arc};

use std::io::Write;

use maudio_sys::ffi as sys;

use crate::context::ContextInner;
use crate::{device::DeviceInner, engine::EngineInner, Binding, ErrorKinds, MaResult, MaudioError};

/// An independently owned miniaudio log.
///
/// A `Log` can be shared with engines and contexts during their construction.
/// Logs remain associated with the log until they are removed
/// or their corresponding [`LogListener`] is dropped.
///
/// In order to pass a `Log` to a [`Device`](crate::device::Device),
/// a `Log` must first be passed to a [`ContextBuilder`](crate::context::ContextBuilder)
/// which must then be passed to a device via [`DeviceBuilder::context`](crate::device::device_builder::DeviceBuilderOps::context)
pub struct Log(pub(crate) Arc<LogInner>);

#[doc(hidden)]
pub struct LogInner {
    inner: *mut sys::ma_log,
    logs: StoredLogs,
}

unsafe impl Send for LogInner {}
unsafe impl Sync for LogInner {}

impl Binding for Log {
    type Raw = *mut sys::ma_log;

    fn to_raw(&self) -> Self::Raw {
        self.0.inner
    }
}

impl Log {
    /// Creates a new log with no registered callbacks.
    pub fn new() -> MaResult<Self> {
        let mut mem: Box<MaybeUninit<sys::ma_log>> = Box::new(MaybeUninit::uninit());

        log_ffi::ma_log_init(mem.as_mut_ptr())?;

        let inner: *mut sys::ma_log = Box::into_raw(mem) as *mut sys::ma_log;
        Ok(Self(Arc::new(LogInner {
            inner,
            logs: StoredLogs::default(),
        })))
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub enum LogOwner {
    Engine(Arc<EngineInner>),
    Device(Arc<DeviceInner>),
    Context(Arc<ContextInner>),
    Log(Arc<LogInner>),
}

/// A reference to the log owned by an engine or context.
///
/// The originating engine, context or device is kept alive for as long as this value
/// exists.
pub struct LogRef {
    pub(crate) inner: *mut sys::ma_log,
    pub(crate) _owner: LogOwner,
}

impl Binding for LogRef {
    type Raw = *mut sys::ma_log;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

mod private_log {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait LogPtrProvider<T: ?Sized> {
        fn as_log_ptr(t: &T) -> *mut sys::ma_log;
        fn clone_owner(t: &T) -> LogOwner;
    }

    pub struct LogProvider;
    pub struct LogRefProvider;

    impl LogPtrProvider<Log> for LogProvider {
        fn as_log_ptr(t: &Log) -> *mut sys::ma_log {
            t.to_raw()
        }

        fn clone_owner(t: &Log) -> LogOwner {
            LogOwner::Log(t.0.clone())
        }
    }

    impl LogPtrProvider<LogRef> for LogRefProvider {
        fn as_log_ptr(t: &LogRef) -> *mut sys::ma_log {
            t.to_raw()
        }

        fn clone_owner(t: &LogRef) -> LogOwner {
            t._owner.clone()
        }
    }

    pub fn log_ptr<T: AsLogPtr + ?Sized>(t: &T) -> *mut sys::ma_log {
        <T as AsLogPtr>::__PtrProvider::as_log_ptr(t)
    }

    pub fn clone_owner<T: AsLogPtr + ?Sized>(t: &T) -> LogOwner {
        <T as AsLogPtr>::__PtrProvider::clone_owner(t)
    }
}

#[doc(hidden)]
pub trait AsLogPtr {
    type __PtrProvider: private_log::LogPtrProvider<Self>;
}

impl AsLogPtr for Log {
    type __PtrProvider = private_log::LogProvider;
}

impl AsLogPtr for LogRef {
    type __PtrProvider = private_log::LogRefProvider;
}

impl<T: AsLogPtr + ?Sized> LogOps for T {}

#[derive(Default)]
pub(crate) struct StoredLogs(OnceLock<Box<Mutex<[Option<LogDefaultRegistration>; 4]>>>);

pub trait LogOps: AsLogPtr {
    /// Removes the standard-error callback registered for `level`.
    ///
    /// This only removes the callback installed by [`LogOps::print_level`].
    /// Custom callbacks registered with [`LogOps::register_log`] are removed by
    /// dropping their [`LogListener`].
    fn remove_level(&self, level: LogLevel) -> MaResult<()> {
        let owner = private_log::clone_owner(self);
        let old = match owner {
            LogOwner::Device(d) => {
                let logs = d
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].take()
            }
            LogOwner::Engine(e) => {
                let logs = e
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].take()
            }
            LogOwner::Context(c) => {
                let logs = c
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].take()
            }
            LogOwner::Log(l) => {
                let logs = l
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].take()
            }
        };
        if let Some(old) = old {
            let _ = log_ffi::ma_log_unregister_callback(private_log::log_ptr(self), old.callback);
        };
        Ok(())
    }

    /// Prints messages of `level` to standard error.
    ///
    /// Calling this again for the same level replaces the previously installed
    /// printing callback.
    fn print_level(&self, level: LogLevel) -> MaResult<()> {
        let callback = match level {
            LogLevel::Debug => log_ffi::ma_log_callback_init(
                Some(ma_log_debug_callback_proc),
                std::ptr::null_mut(),
            ),
            LogLevel::Info => {
                log_ffi::ma_log_callback_init(Some(ma_log_info_callback_proc), std::ptr::null_mut())
            }
            LogLevel::Warning => log_ffi::ma_log_callback_init(
                Some(ma_log_warning_callback_proc),
                std::ptr::null_mut(),
            ),
            LogLevel::Error => log_ffi::ma_log_callback_init(
                Some(ma_log_error_callback_proc),
                std::ptr::null_mut(),
            ),
        };

        log_ffi::ma_log_register_callback(self, callback)?;

        let cb = LogDefaultRegistration { callback };

        let owner = private_log::clone_owner(self);
        let old = match owner {
            LogOwner::Device(d) => {
                let logs = d
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].replace(cb)
            }
            LogOwner::Engine(e) => {
                let logs = e
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].replace(cb)
            }
            LogOwner::Context(c) => {
                let logs = c
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].replace(cb)
            }
            LogOwner::Log(l) => {
                let logs = l
                    .logs
                    .0
                    .get_or_init(|| Box::new(Mutex::new(std::array::from_fn(|_| None))));
                logs.lock().unwrap()[level.index()].replace(cb)
            }
        };
        if let Some(old) = old {
            // unregister it an old one exists
            let _ = log_ffi::ma_log_unregister_callback(private_log::log_ptr(self), old.callback);
        };
        Ok(())
    }

    /// Registers a callback that receives log messages from all log levels.
    ///
    /// The returned [`LogListener`] controls the lifetime of the registration.
    /// Dropping it unregisters the callback. The listener must therefore be kept
    /// alive for as long as messages should be received.
    fn register_log<C>(&self, callback: C) -> MaResult<LogListener>
    where
        C: Fn(LogLevel, &str) + Send + 'static,
    {
        let user_data = LogUserRegistration { user_cb: callback };
        let erased = Erased::new(user_data);

        let callback =
            unsafe { sys::ma_log_callback_init(Some(ma_log_user_callback_proc::<C>), erased.ptr) };

        log_ffi::ma_log_register_callback(self, callback)?;

        Ok(LogListener {
            log: private_log::log_ptr(self),
            _user_data: erased,
            callback,
            _owner: private_log::clone_owner(self),
        })
    }
}

/// A handle to a registered log callback.
///
/// Dropping this handle unregisters the callback.
#[must_use = "dropping the listener immediately unregisters the callback"]
pub struct LogListener {
    log: *mut sys::ma_log,
    _user_data: Erased,
    callback: sys::ma_log_callback,
    _owner: LogOwner,
}

impl Drop for LogListener {
    fn drop(&mut self) {
        let _ = log_ffi::ma_log_unregister_callback(self.log, self.callback);
    }
}

mod log_ffi {
    use maudio_sys::ffi as sys;

    use crate::{
        logging::{private_log, AsLogPtr, Log, LogLevel},
        AllocationCallbacks, Binding, MaResult, MaudioError,
    };

    #[inline]
    pub fn ma_log_init(log: *mut sys::ma_log) -> MaResult<()> {
        let res = unsafe { sys::ma_log_init(AllocationCallbacks::cb_ptr(), log) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_log_uninit(log: &mut Log) {
        unsafe {
            sys::ma_log_uninit(log.to_raw());
        }
    }

    #[inline]
    pub fn ma_log_register_callback<L: AsLogPtr + ?Sized>(
        log: &L,
        cb: sys::ma_log_callback,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_log_register_callback(private_log::log_ptr(log), cb) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_log_unregister_callback(
        log: *mut sys::ma_log,
        cb: sys::ma_log_callback,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_log_unregister_callback(log, cb) };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_log_callback_init(
        on_log: sys::ma_log_callback_proc,
        user_data: *mut core::ffi::c_void,
    ) -> sys::ma_log_callback {
        unsafe { sys::ma_log_callback_init(on_log, user_data) }
    }

    // There is no use case for this yet
    #[inline]
    pub fn _ma_log_post(log: &Log, level: LogLevel, message: *const i8) -> MaResult<()> {
        let res = unsafe { sys::ma_log_post(log.to_raw(), level.into(), message) };
        MaudioError::check(res)
    }
}

impl Drop for Log {
    fn drop(&mut self) {
        log_ffi::ma_log_uninit(self);
        drop(unsafe { Box::from_raw(self.to_raw()) });
    }
}

struct LogUserRegistration<C>
where
    C: Fn(LogLevel, &str) + Send + 'static,
{
    user_cb: C,
    // a panic flag?
}

struct LogDefaultRegistration {
    callback: sys::ma_log_callback,
}

unsafe extern "C" fn ma_log_user_callback_proc<C>(
    user_data: *mut core::ffi::c_void,
    level: u32,
    message: *const core::ffi::c_char,
) where
    C: Fn(LogLevel, &str) + Send + 'static,
{
    if message.is_null() || user_data.is_null() {
        return;
    }

    let Ok(level): Result<LogLevel, _> = level.try_into() else {
        return;
    };

    let Ok(message) = std::ffi::CStr::from_ptr(message).to_str() else {
        return;
    };

    let message = message
        .strip_suffix("\r\n")
        .or_else(|| message.strip_suffix('\n'))
        .unwrap_or(message);

    let registration = &*user_data.cast::<LogUserRegistration<C>>();

    // TODO: Add a poison flag?
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| (registration.user_cb)(level, message)));
}

unsafe extern "C" fn ma_log_debug_callback_proc(
    _user_data: *mut core::ffi::c_void,
    level: u32,
    message: *const core::ffi::c_char,
) {
    // Cast is safe, but needed on windows
    #[allow(clippy::unnecessary_cast)]
    if level != sys::ma_log_level_MA_LOG_LEVEL_DEBUG as u32 {
        return;
    }

    if message.is_null() {
        return;
    }

    let Ok(message) = std::ffi::CStr::from_ptr(message).to_str() else {
        return;
    };

    let message = message
        .strip_suffix("\r\n")
        .or_else(|| message.strip_suffix('\n'))
        .unwrap_or(message);

    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "[DEBUG] {message}");
}

unsafe extern "C" fn ma_log_info_callback_proc(
    _user_data: *mut core::ffi::c_void,
    level: u32,
    message: *const core::ffi::c_char,
) {
    // Cast is safe, but needed on windows
    #[allow(clippy::unnecessary_cast)]
    if level != sys::ma_log_level_MA_LOG_LEVEL_INFO as u32 {
        return;
    }

    if message.is_null() {
        return;
    }

    let Ok(message) = std::ffi::CStr::from_ptr(message).to_str() else {
        return;
    };

    let message = message
        .strip_suffix("\r\n")
        .or_else(|| message.strip_suffix('\n'))
        .unwrap_or(message);

    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "[INFO] {message}");
}

unsafe extern "C" fn ma_log_warning_callback_proc(
    _user_data: *mut core::ffi::c_void,
    level: u32,
    message: *const core::ffi::c_char,
) {
    // Cast is safe, but needed on windows
    #[allow(clippy::unnecessary_cast)]
    if level != sys::ma_log_level_MA_LOG_LEVEL_WARNING as u32 {
        return;
    }

    if message.is_null() {
        return;
    }

    let Ok(message) = std::ffi::CStr::from_ptr(message).to_str() else {
        return;
    };

    let message = message
        .strip_suffix("\r\n")
        .or_else(|| message.strip_suffix('\n'))
        .unwrap_or(message);

    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "[WARNING] {message}");
}

unsafe extern "C" fn ma_log_error_callback_proc(
    _user_data: *mut core::ffi::c_void,
    level: u32,
    message: *const core::ffi::c_char,
) {
    // Cast is safe, but needed on windows
    #[allow(clippy::unnecessary_cast)]
    if level != sys::ma_log_level_MA_LOG_LEVEL_ERROR as u32 {
        return;
    }

    if message.is_null() {
        return;
    }

    let Ok(message) = std::ffi::CStr::from_ptr(message).to_str() else {
        return;
    };

    let message = message
        .strip_suffix("\r\n")
        .or_else(|| message.strip_suffix('\n'))
        .unwrap_or(message);

    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "[ERROR] {message}");
}

pub(crate) struct Erased {
    ptr: *mut c_void,
    drop_fn: unsafe fn(*mut c_void),
}

impl Erased {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        let boxed = Box::new(value);

        unsafe fn drop_impl<T>(ptr: *mut c_void) {
            drop(Box::from_raw(ptr.cast::<T>()));
        }

        Self {
            ptr: Box::into_raw(boxed).cast(),
            drop_fn: drop_impl::<T>,
        }
    }
}

impl Drop for Erased {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.ptr) }
    }
}

/// Miniaudio log message severity.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    fn index(&self) -> usize {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "Debug"),
            LogLevel::Info => write!(f, "Info"),
            LogLevel::Warning => write!(f, "Warning"),
            LogLevel::Error => write!(f, "Error"),
        }
    }
}

impl TryFrom<sys::ma_log_level> for LogLevel {
    type Error = MaudioError;

    fn try_from(
        value: sys::ma_log_level,
    ) -> Result<Self, <Self as TryFrom<sys::ma_log_level>>::Error> {
        match value {
            sys::ma_log_level_MA_LOG_LEVEL_DEBUG => Ok(LogLevel::Debug),
            sys::ma_log_level_MA_LOG_LEVEL_INFO => Ok(LogLevel::Info),
            sys::ma_log_level_MA_LOG_LEVEL_WARNING => Ok(LogLevel::Warning),
            sys::ma_log_level_MA_LOG_LEVEL_ERROR => Ok(LogLevel::Error),
            other => Err(MaudioError::new_ma_error(ErrorKinds::unknown_enum::<
                LogLevel,
            >(other as i64))),
        }
    }
}

impl From<LogLevel> for sys::ma_log_level {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Debug => sys::ma_log_level_MA_LOG_LEVEL_DEBUG,
            LogLevel::Info => sys::ma_log_level_MA_LOG_LEVEL_INFO,
            LogLevel::Warning => sys::ma_log_level_MA_LOG_LEVEL_WARNING,
            LogLevel::Error => sys::ma_log_level_MA_LOG_LEVEL_ERROR,
        }
    }
}

#[cfg(test)]
mod test {
    #[cfg(not(feature = "ci-tests"))]
    use crate::{
        context::{ContextBuilder, ContextOps},
        device::device_builder::{DeviceBuilder, DeviceBuilderOps},
        engine::engine_builder::EngineBuilder,
        logging::{Log, LogLevel, LogOps},
    };

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_engine_print_all() {
        let log = Log::new().unwrap();
        log.print_level(LogLevel::Info).unwrap();
        log.print_level(LogLevel::Debug).unwrap();
        log.print_level(LogLevel::Warning).unwrap();
        log.print_level(LogLevel::Error).unwrap();

        let _engine = EngineBuilder::new().logger(&log).build_for_tests().unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_device_print_all() {
        let log = Log::new().unwrap();
        log.print_level(LogLevel::Info).unwrap();
        log.print_level(LogLevel::Debug).unwrap();
        log.print_level(LogLevel::Warning).unwrap();
        log.print_level(LogLevel::Error).unwrap();

        let mut ctx = ContextBuilder::new();
        let ctx = ctx.log(&log);

        let _device = DeviceBuilder::playback()
            .f32()
            .context(ctx)
            .with_callback(|_, a| a.fill(0.0))
            .unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_engine_log_reg_print_all() {
        let engine = EngineBuilder::new().build_for_tests().unwrap();

        let log = engine.log();

        log.print_level(LogLevel::Info).unwrap();
        log.print_level(LogLevel::Debug).unwrap();
        log.print_level(LogLevel::Warning).unwrap();
        log.print_level(LogLevel::Error).unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_device_log_reg_print_all() {
        let device = DeviceBuilder::playback()
            .f32()
            .with_callback(|_, a| a.fill(0.0))
            .unwrap();

        let log = device.log();

        log.print_level(LogLevel::Info).unwrap();
        log.print_level(LogLevel::Debug).unwrap();
        log.print_level(LogLevel::Warning).unwrap();
        log.print_level(LogLevel::Error).unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_engine_register_without_drop() {
        let log = Log::new().unwrap();
        let _listener = log.register_log(|_, msg| println!("{msg}")).unwrap();

        let _engine = EngineBuilder::new().logger(&log).build_for_tests().unwrap();
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_device_register_without_drop() {
        let log = Log::new().unwrap();
        let _listener = log.register_log(|_, msg| println!("{msg}")).unwrap();

        let mut ctx = ContextBuilder::new();
        let ctx = ctx.log(&log);

        let _device = DeviceBuilder::playback()
            .f32()
            .context(ctx)
            .with_callback(|_, a| a.fill(0.0))
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    #[cfg(not(feature = "ci-tests"))]
    #[test]
    fn logger_test_context_check() {
        let log = Log::new().unwrap();
        let _listener = log.register_log(|_, msg| println!("{msg}")).unwrap();

        let ctx = ContextBuilder::new().log(&log).build().unwrap();

        ctx.enumerate_devices(|_, d| {
            println!("{}", d.name());
            crate::context::EnumerateControl::Stop
        })
        .unwrap();
    }
}
