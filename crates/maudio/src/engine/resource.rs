//! Resource manager
//!
//! The resource manager is Miniaudio’s asynchronous asset loader/decoder front-end.
//! It is designed to be used from **multiple threads**: internally it uses a
//! multi-producer / multi-consumer job queue plus background worker threads.
//!
//! ## Relationship to the Engine
//!
//! An [`Engine`](crate::engine) will always have an underlying resource manager.
//!
//! The API exposed by the resource manager directly offers functionality that is not 100%
//! matching how the engine may use a resource manager.
//!
//! Primarily, the `Engine` uses the it with sounds that are not already loaded in memory, and
//! reference counts them if they are used multiple times.
//!
//! When working through the engine, [`ResourceGuard`] and explicit resource
//! registration are not necessary. The engine handles acquiring and
//! releasing resources internally.
//!
//! The resource manager can also be used independently of the engine through
//! the low-level API. In this case, it can be used as a general-purpose store
//! for registering and loading resources, including resources already held in
//! memory, and for sharing those resources across threads.
//!
//! In `maudio`, the resource manager is exposed in two forms:
//!
//! - [`ResourceManager`]: an owned, reference-counted handle that can be cloned.
//! - [`ResourceManagerRef`]: a non-owning reference primarily returned by other objects (e.g. the engine).
//!
//! ### PCM format choice
//!
//! A resource mananger contains a decoder with a format converted, resampler and channel converter.
//! When building a resource manager you are asked to select the `output` format.
//!
//! Sounds loaded into the resource manager will be converted to this outpuf format if it differs.
//! A [`ResourceGuard`] can show both input (native) and output format if both are known.
//! A decoded, i16 sound loaded into an f32 ResourceManager will show:
//! ```ignore
//! ResourceGuard<f32, i16>
//! ```
//! If you are loading an encoded sound, it will simply show:
//! ```ignore
//! ResourceGuard<f32, Unknown>
//! ```
//!
//! ## What is the resource manager (in Miniaudio)?
//!
//! Miniaudio’s resource manager (`ma_resource_manager`) is a high-level facility for
//! **loading audio assets** (files or in memory sounds) and exposing them as a standard
//! `ma_data_source`.
//!
//! Conceptually, it sits between “where the bytes come from” and “who plays/consumes
//! PCM frames”:
//!
//! - **I/O**: loads from files
//! - **Decoding**: can decode compressed formats into PCM, and can do this
//!   **asynchronously** using background worker threads.
//! - **Streaming vs. fully loaded**: can either stream from the source, or load the
//!   whole asset into memory (optionally decoded/uncompressed) depending on flags.
//! - **Uniform interface**: the thing you get back is “just a data source”, which means it
//!   plugs into the rest of Miniaudio anywhere a `ma_data_source` is accepted.
//!
//! In `maudio`, the resource manager primarily exists so you can:
//! - share one loader/decoder across many sounds,
//! - keep decoding/loading work off the audio callback thread,
//! - and unify “load from file / load from memory” behind the same typed API.
//! - loading a sound in a thread, and retrieving a data source for it in another
//!
//! ### Sharing a resource manager between multiple engines
//!
//! A resource manager is **not tied to a single engine**. In Miniaudio, an engine
//! can be configured to use an externally created `ma_resource_manager`, but it
//! does not own it.
//!
//! The same design applies in `maudio`:
//!
//! - You can create one [`ResourceManager<F>`].
//! - Attach it to multiple [`Engine`](crate::engine::Engine) instances.
//! - All engines will share the same loader, decoder threads, and asset cache.
//!
//! This is particularly useful when:
//!
//! - You have multiple output devices (e.g. different audio backends or test engines).
//! - You want a single shared asset cache across subsystems.
//! - You want to centralize asynchronous loading/decoding.
//!
//! ### Typical usage
//!
//! Create a resource manager, then attach it to an engine:
//!
//! ```rust,no_run
//! # use maudio::engine::engine_builder::EngineBuilder;
//! # use maudio::engine::resource::rm_builder::ResourceManagerBuilder;
//! # fn main() -> maudio::MaResult<()> {
//! let rm = ResourceManagerBuilder::new_f32().build()?;
//! let engine = EngineBuilder::new()
//!     .resource_manager(&rm)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! If you already have an engine, you can access its resource manager via
//! `engine.resource_manager()` which returns a borrowed [`ResourceManagerRef`].
//!
use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    path::{Path, PathBuf},
    sync::Arc,
};

use maudio_sys::ffi as sys;

use crate::{
    audio::{formats::Format, sample_rate::SampleRate},
    data_source::{
        sources::decoder::custom_decoder::BackendRegistration, AsSourcePtr, SharedSource,
    },
    device::device_builder::Unknown,
    engine::{
        resource::{
            rm_buffer::{ResourceManagerBuffer, ResourceManagerBufferBuilder},
            rm_builder::ResourceManagerBuilder,
            rm_notif::NotificationPipeline,
            rm_source::{ResourceManagerSource, ResourceManagerSourceBuilder},
            rm_source_flags::RmSourceFlags,
            rm_stream::{ResourceManagerStream, ResourceManagerStreamBuilder},
        },
        EngineInner,
    },
    pcm_frames::{PcmFormat, S24Packed},
    test_assets::wav_i16_le,
    AsRawRef, Binding, MaResult, MaudioError,
};

pub mod rm_buffer;
pub mod rm_builder;
pub mod rm_flags;
pub mod rm_notif;
pub mod rm_source;
pub mod rm_source_flags;
pub mod rm_stream;

/// An owned resource manager handle.
///
/// Besides configuring background loading/decoding, the resource manager also acts as a
/// **registry** for named resources. Registering a resource lets you later create
/// resource-backed data sources (buffer/stream/source) using Miniaudio’s internal cache
/// and job system.
///
/// ## How to create resource-backed audio
///
/// A sound is added to the ResourceManager via a registration.
/// The registration process returns a guard that keeps that registration alive.
/// When all guards for this sound are dropped, the sound is removed from the
/// ResourceManager and can no longer be used.
///
/// You get the biggest benefit out of the resource manager when you are registering
/// sounds that are not already in memory.
///
/// However, the ResourceManager does support the following ways to register sounds:
/// ```text
/// register_file                 - takes &Path
/// register_encoded              - in memory, raw bytes
/// register_decoded_u8           - in memory, u8 native decoded
/// register_decoded_i16          - in memory, i16 native decoded
/// register_decoded_i32          - in memory, i32 native decoded
/// register_decoded_s24_packed   - in memory, 3-byte packed native decoded
/// register_decoded_f32          - in memory, f32 native decoded
///```
///
/// When registering sounds, the ResourceManager acts similarly to a key-value store.
/// Each registration also requirs a name for that resource. For file backed registration,
/// the path provided becomes the name.
///
/// After the registration, maudio exposes 2 functions which also return a `ResourceGuard`:
/// ```text
/// load_name()
/// load_path()
/// ```
/// These functions will fail with `MA_INVALID_ARGS`, if the name or path has not been registered.
/// Since the ResourceManager is thread safe, this makes it easy to register sounds in a thread,
/// and load a data source for that sound in another thread.
///
/// From the returned guard you can build:
/// - [`ResourceManagerBuffer`]: fully buffered, typically decoded into memory.
/// - [`ResourceManagerStream`]: streamed decode / read-ahead. Cannot be used for in memory sounds.
/// - [`ResourceManagerSource`]: generic data source.
///
/// ### Thread safety
///
/// The following types are Send + Sync:
/// - [`ResourceManager`]
/// - [`ResourceManagerRef`]
/// - [`ResourceGuard`]
///
/// Miniaudio documents `ma_resource_manager` as safe to use from multiple threads:
/// it uses a multi-producer/multi-consumer job queue and background job threads.
///
///
/// ### Multiple engine support
///
/// A single `ResourceManager<F>` can be attached to multiple engines.
/// The underlying Miniaudio resource manager is independent of the engine,
/// and sharing it allows multiple engines to reuse the same job threads
/// and asset cache.
///
/// Use [`ResourceManagerBuilder`] to initialize.
#[derive(Clone)]
pub struct ResourceManager<F: PcmFormat>(Arc<ResourceManagerInner<F>>);

#[doc(hidden)]
#[derive(Debug)]
pub struct ResourceManagerInner<F: PcmFormat> {
    inner: *mut sys::ma_resource_manager,
    #[allow(unused)]
    channels: Option<u32>,
    #[allow(unused)]
    format: Format,
    backend_reg: Option<*mut BackendRegistration<F>>,
    _decoder_vtables: Box<[*const sys::ma_decoding_backend_vtable]>, // keep alive
    _format: PhantomData<F>,
}

unsafe impl<F: PcmFormat> Send for ResourceManager<F> {}
unsafe impl<F: PcmFormat> Sync for ResourceManager<F> {}

impl<F: PcmFormat> Binding for ResourceManager<F> {
    type Raw = *mut sys::ma_resource_manager;

    fn to_raw(&self) -> Self::Raw {
        self.0.inner
    }
}

/// A borrowed resource manager.
///
/// This is a lightweight non-owning view into an existing resource manager.
/// It is primarily returned by other objects (for example `Engine::resource_manager()`).
///
pub struct ResourceManagerRef<F: PcmFormat> {
    pub(crate) inner: *mut sys::ma_resource_manager,
    pub(crate) _format: PhantomData<F>,
    pub(crate) owner: RmOwner<F>,
}

unsafe impl<F: PcmFormat> Send for ResourceManagerRef<F> {}
unsafe impl<F: PcmFormat> Sync for ResourceManagerRef<F> {}

impl<F: PcmFormat> Binding for ResourceManagerRef<F> {
    type Raw = *mut sys::ma_resource_manager;

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub(crate) mod private_rm {
    use super::*;
    use maudio_sys::ffi as sys;

    pub trait RmPtrProvider<F: PcmFormat, T: ?Sized> {
        fn as_rm_ptr(t: &T) -> *mut sys::ma_resource_manager;
        fn clone_owner(t: &T) -> RmOwner<F>;
    }

    pub struct RmProvider;
    pub struct RmRefProvider;

    impl<F: PcmFormat> RmPtrProvider<F, ResourceManager<F>> for RmProvider {
        fn as_rm_ptr(t: &ResourceManager<F>) -> *mut sys::ma_resource_manager {
            t.to_raw()
        }

        fn clone_owner(t: &ResourceManager<F>) -> RmOwner<F> {
            RmOwner::Rm(t.0.clone())
        }
    }

    impl<F: PcmFormat> RmPtrProvider<F, ResourceManagerRef<F>> for RmRefProvider {
        fn as_rm_ptr(t: &ResourceManagerRef<F>) -> *mut sys::ma_resource_manager {
            t.to_raw()
        }

        fn clone_owner(t: &ResourceManagerRef<F>) -> RmOwner<F> {
            t.owner.clone()
        }
    }

    pub fn rm_ptr<T: AsRmPtr + ?Sized>(t: &T) -> *mut sys::ma_resource_manager {
        <T as AsRmPtr>::__PtrProvider::as_rm_ptr(t)
    }

    pub fn clone_owner<T: AsRmPtr + ?Sized>(t: &T) -> RmOwner<<T as AsRmPtr>::Format> {
        <T as AsRmPtr>::__PtrProvider::clone_owner(t)
    }
}

#[doc(hidden)]
pub trait AsRmPtr {
    type __PtrProvider: private_rm::RmPtrProvider<Self::Format, Self>;
    type Format: PcmFormat;
}

impl<F: PcmFormat> AsRmPtr for ResourceManager<F> {
    type __PtrProvider = private_rm::RmProvider;
    type Format = F;
}

impl<F: PcmFormat> AsRmPtr for ResourceManagerRef<F> {
    type __PtrProvider = private_rm::RmRefProvider;
    type Format = F;
}

#[doc(hidden)]
pub enum RmOwner<F: PcmFormat> {
    Engine(Arc<EngineInner>),
    Rm(Arc<ResourceManagerInner<F>>),
}

impl<F: PcmFormat> Clone for RmOwner<F> {
    fn clone(&self) -> Self {
        match self {
            Self::Engine(e) => RmOwner::Engine(e.clone()),
            Self::Rm(r) => RmOwner::Rm(r.clone()),
        }
    }
}

/// Keeps a resource manager registration alive.
///
/// A `ResourceGuard` is returned by `ResourceManager::register_*()` calls.
/// While the guard exists, the registered resource remains available to the resource manager,
/// and you can create one or more data [`ResourceManagerBuffer`], [`ResourceManagerStream`], or
/// [`ResourceManagerSource`] values from it via `build_buffer/build_stream/build_source`.
///
/// This is the primary workflow for memory-backed resources, including audio
/// data that already exists in memory before being handed to the resource
/// manager.
///
/// The guard may also hold ownership of the underlying bytes (for conversions done by maudio).
///
/// Dropping the guard unregisters the resource once it is no longer in active use.
///
/// A guard can show both the native and the output format of the backing audio.
/// A decoded, i16 sound loaded into an f32 ResourceManager will show:
/// ```ignore
/// ResourceGuard<f32, i16>
/// ```
/// If you are loading an encoded sound, it will simply show:
/// ```ignore
/// ResourceGuard<f32, Unknown>
/// ```
#[derive(Clone)]
pub struct ResourceGuard<F: PcmFormat, I>(pub(crate) Arc<ResourceGuardInner<F, I>>);

pub(crate) struct ResourceGuardInner<F: PcmFormat, I> {
    rm: *mut sys::ma_resource_manager,
    data_name: RegisteredDataName,
    _data_store: Option<RegisteredData<I>>,
    owner: RmOwner<F>,
}

unsafe impl<F: PcmFormat, I> Send for ResourceGuard<F, I> {}
unsafe impl<F: PcmFormat, I> Sync for ResourceGuard<F, I> {}

// Builders for registered resources
impl<F: PcmFormat, I> ResourceGuard<F, I> {
    /// Returns the reference to the resource manager.
    pub fn resource_manager(&self) -> ResourceManagerRef<F> {
        ResourceManagerRef {
            inner: self.0.rm,
            _format: PhantomData,
            owner: self.0.owner.clone(),
        }
    }

    /// Builds a [`ResourceManagerBuffer`] from a previously registered file path or
    /// in memory resource.
    ///
    /// This does not itself register raw in-memory audio data. For memory-backed
    /// resources, register them first and then use [`ResourceGuard::build_buffer`].
    /// ## Flags:
    ///
    /// - [`RmSourceFlags::LOOPING`] to enable looping
    /// - [`RmSourceFlags::WAIT_INIT`]
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   initial async initialization/prefill step completes before returning.
    /// - [`RmSourceFlags::ASYNC`]
    ///   Initialize and prefill the stream asynchronously using the resource manager’s worker threads.
    /// - [`RmSourceFlags::STREAM`] flag is ignored by maudio
    pub fn build_buffer(
        &self,
        flags: RmSourceFlags,
        notif: Option<NotificationPipeline>,
    ) -> MaResult<PendingResource<ResourceManagerBuffer<F, I>>> {
        let mut builder = ResourceManagerBufferBuilder::new(self);

        let mut flags_check = flags;
        if flags_check.intersects(RmSourceFlags::STREAM) {
            flags_check.remove(RmSourceFlags::STREAM);
        }
        builder.flags(flags_check);

        if let Some(notif) = notif {
            builder.notification(notif);
        }

        match &self.0.data_name {
            RegisteredDataName::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataName::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        let resource = builder.build_internal()?;
        if flags_check.intersects(RmSourceFlags::ASYNC) {
            return Ok(PendingResource::Pending {
                inner: Some(resource),
            });
        }
        Ok(PendingResource::Ready { inner: resource })
    }

    /// Builds a [`ResourceManagerStream`] for streaming a file-backed resource.
    ///
    /// This builder creates a stream from a resource that was
    /// previously registered from a file-backed input.
    ///
    /// Streaming is intended for resources that can be read on demand by the
    /// resource manager. It does not register new data with the resource manager.
    ///
    /// In-memory registrations are not suitable for streaming. If a resource was
    /// registered from in-memory data (decoded or encoded), [`ResourceGuard::build_stream`]
    /// is not supported and will return a `MA_DOES_NOT_EXIST` error.
    ///
    /// ## Flags:
    ///
    /// - [`RmSourceFlags::LOOPING`] to enable looping
    /// - [`RmSourceFlags::WAIT_INIT`]
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   initial async initialization/prefill step completes before returning.
    /// - [`RmSourceFlags::ASYNC`]
    ///   Initialize and prefill the stream asynchronously using the resource manager’s worker threads.
    /// - [`RmSourceFlags::STREAM`] flag is ignored by maudio
    pub fn build_stream(
        &self,
        flags: RmSourceFlags,
        notif: Option<NotificationPipeline>,
    ) -> MaResult<PendingResource<ResourceManagerStream<F, I>>> {
        let mut builder = ResourceManagerStreamBuilder::new(self);
        let mut flags_check = flags;
        if !flags_check.intersects(RmSourceFlags::STREAM) {
            flags_check.insert(RmSourceFlags::STREAM);
        }
        builder.flags(flags_check);

        if let Some(notif) = notif {
            builder.notification(notif);
        }

        match &self.0.data_name {
            RegisteredDataName::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataName::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        let resource = builder.build_internal()?;
        if flags_check.intersects(RmSourceFlags::ASYNC) {
            return Ok(PendingResource::Pending {
                inner: Some(resource),
            });
        }
        Ok(PendingResource::Ready { inner: resource })
    }

    /// Builds a [`ResourceManagerSource`] from a previously registered file path or
    /// in memory resource.
    ///
    /// This does not itself register raw in-memory audio data. For memory-backed
    /// resources, register them first and then use [`ResourceGuard::build_source`].
    /// ## Flags:
    /// Under the hood, this type is either `ResourceManagerBuffer` or `ResourceManagerStream`.
    ///
    /// `ResourceManagerSource` is an abstraction and underlying type can be chosen using the `STREAM` flag.
    ///
    /// - [`RmSourceFlags::LOOPING`] to enable looping
    /// - [`RmSourceFlags::WAIT_INIT`]
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   initial async initialization/prefill step completes before returning.
    /// - [`RmSourceFlags::ASYNC`]
    ///   Initialize and prefill the stream asynchronously using the resource manager’s worker threads.
    /// - [`RmSourceFlags::STREAM`]
    ///   If this flag is passed, this source will be initialized as a `ResourceManagerStream`.
    ///   Without this flag, it be initialized as a `ResourceManagerBuffer`
    pub fn build_source(
        &self,
        flags: RmSourceFlags,
        notif: Option<NotificationPipeline>,
    ) -> MaResult<PendingResource<ResourceManagerSource<F, I>>> {
        let mut builder = ResourceManagerSourceBuilder::new(self);

        builder.flags(flags);

        if let Some(notif) = notif {
            builder.notification(notif);
        }

        match &self.0.data_name {
            RegisteredDataName::RegisteredPath { path } => builder.file_path(path),
            RegisteredDataName::RegisteredData { name } => builder.file_path(Path::new(name)),
        };
        let resource = builder.build_internal()?;
        if flags.intersects(RmSourceFlags::ASYNC) {
            return Ok(PendingResource::Pending {
                inner: Some(resource),
            });
        }
        Ok(PendingResource::Ready { inner: resource })
    }
}

// Private methods
impl<F: PcmFormat, I> ResourceGuard<F, I> {
    #[inline]
    pub(crate) fn from_path<R: AsRmPtr<Format = F> + ?Sized>(
        rm: &R,
        path: &Path,
    ) -> ResourceGuard<F, I> {
        ResourceGuard(Arc::new(ResourceGuardInner {
            rm: private_rm::rm_ptr(rm),
            data_name: RegisteredDataName::RegisteredPath {
                path: path.to_path_buf(),
            },
            _data_store: None,
            owner: private_rm::clone_owner::<R>(rm),
        }))
    }

    pub(crate) fn from_encoded_data<R: AsRmPtr<Format = F> + ?Sized>(
        rm: &R,
        name: &str,
        data: impl Into<Arc<[u8]>>,
    ) -> ResourceGuard<F, I> {
        ResourceGuard(Arc::new(ResourceGuardInner {
            rm: private_rm::rm_ptr(rm),
            data_name: RegisteredDataName::RegisteredData {
                name: name.to_string(),
            },
            _data_store: Some(RegisteredData::Encoded(data.into())),
            owner: private_rm::clone_owner::<R>(rm),
        }))
    }

    pub(crate) fn from_decoded_data<R: AsRmPtr<Format = F> + ?Sized>(
        rm: &R,
        name: &str,
        data: impl Into<Arc<[I]>>,
    ) -> ResourceGuard<F, I> {
        ResourceGuard(Arc::new(ResourceGuardInner {
            rm: private_rm::rm_ptr(rm),
            data_name: RegisteredDataName::RegisteredData {
                name: name.to_string(),
            },
            _data_store: Some(RegisteredData::<I>::Decoded(data.into())),
            owner: private_rm::clone_owner::<R>(rm),
        }))
    }
}

impl<F: PcmFormat, I> Drop for ResourceGuardInner<F, I> {
    fn drop(&mut self) {
        match &self.data_name {
            RegisteredDataName::RegisteredData { name } => {
                let _ = resource_ffi::ma_resource_manager_unregister_data_internal(self.rm, name);
            }
            RegisteredDataName::RegisteredPath { path } => {
                let _ = resource_ffi::ma_resource_manager_unregister_file_internal(self.rm, path);
            }
        }
    }
}

// We never read this. It simply keeps the data alive.
#[allow(dead_code)]
enum RegisteredData<I> {
    Encoded(Arc<[u8]>),
    Decoded(Arc<[I]>),
}

enum RegisteredDataName {
    RegisteredPath { path: PathBuf },
    RegisteredData { name: String },
}

/// Result of building a resource-manager data source.
///
/// When asynchronous loading is enabled, the resource may not be ready
/// immediately. In that case, this type represents the current state
/// of the resource.
///
/// # States
///
/// - [`PendingResource::Ready`]   - The resource is fully initialized and ready for use.
/// - [`PendingResource::Pending`] - The resource is still being loaded or initialized (`MA_BUSY`).
/// - [`PendingResource::Failed`]  - The operation failed with an error.
///
/// # Async workflow
///
/// When async flags are used, builders return [`PendingResource::Pending`].
/// The resource can then be handled in one of two ways:
///
/// ## 1. Polling (this type)
///
/// Call [`PendingResource::poll_ready`] periodically until the resource
/// becomes ready.
///
/// ```ignore
/// # let mut pending = todo!();
/// while !pending.poll_ready()? {
///     // do other work
/// }
///
/// let resource = pending.into_ready().unwrap();
/// ```
///
/// ## 2. Fence-based notification
///
/// Instead of polling, you can attach a [`NotificationPipeline`] with a [`Fence`](crate::util::fence::Fence)
/// to be notified when the resource finishes loading.
///
/// This avoids repeated polling and allows integration with your own
/// synchronization or event systems.
///
/// ```ignore
/// # let rm = todo!();
/// # let path = todo!();
/// let fence = Fence::new();
///
/// let notif = NotificationPipelineBuilder::new().done_with_fence(&fence).build();
///
/// let pending = ResourceManagerBufferBuilder::new(&rm)
///     .notification(notif.clone())
///     .build()?;
///
/// // Block until the resource is fully ready
/// fence.wait()?;
///
/// // Now safe to extract the resource
/// let resource = pending.into_ready().unwrap();
/// ```
///
/// # Notes
///
/// - [`PendingResource`] is the Rust-side state wrapper for the resource.
/// - [`NotificationPipeline`] is an optional signaling mechanism.
/// - These can be used independently or together.
/// - In most cases, users should use a fence on the `done` stage to detect
///   when the resource is fully ready.
pub enum PendingResource<B: AsAsyncSource> {
    /// MA_SUCCESS
    Ready { inner: B },
    /// MA_BUSY
    Pending { inner: Option<B> },
    /// Other Error
    Failed(MaudioError),
}

// TODO: Derive PartialEq for PendingResource?
impl<B: AsAsyncSource> std::fmt::Debug for PendingResource<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready { .. } => f.write_str("PendingResource::Ready(..)"),
            Self::Pending { .. } => f.write_str("PendingResource::Pending(..)"),
            Self::Failed(e) => f.debug_tuple("PendingResource::Failed").field(e).finish(),
        }
    }
}

impl<B: AsAsyncSource> PendingResource<B> {
    /// Polls the resource.
    ///
    /// Returns:
    /// - **Ok(true)** => resource is ready (or was already ready and no polling was done).
    /// - **Ok(false)** => resource pending (BUSY)
    /// - **Err(e)** => Failed (and cached)
    ///
    /// **Do not spin loop. Use [`NotificationPipeline`] instead.**
    pub fn poll_ready(&mut self) -> MaResult<bool> {
        match self {
            PendingResource::Ready { inner: _ } => Ok(true),
            PendingResource::Pending { inner } => {
                if let Some(buf) = inner {
                    match private_async_src::result_check(buf) {
                        Ok(_) => {
                            let inner = inner.take();
                            *self = PendingResource::Ready {
                                inner: inner.unwrap(),
                            }; // unwrap is guaranteed to not panic
                            Ok(true)
                        }
                        Err(e) if e.is_busy() => Ok(false),
                        Err(e) => {
                            *self = PendingResource::Failed(e.clone());
                            Err(e)
                        }
                    }
                } else {
                    unreachable!()
                }
            }
            PendingResource::Failed(e) => Err(e.clone()),
        }
    }

    /// Checks if the resource is available. **Does not poll**.
    pub fn is_ready(&self) -> bool {
        matches!(self, PendingResource::Ready { inner: _ })
    }

    /// Returns a reference to the resource, if ready. **Does not poll**.
    pub fn as_ready(&self) -> Option<&B> {
        match self {
            PendingResource::Ready { inner } => Some(inner),
            _ => None,
        }
    }

    /// Returns a mutable reference to the resource, if ready. **Does not poll**.
    pub fn as_ready_mut(&mut self) -> Option<&mut B> {
        match self {
            PendingResource::Ready { inner } => Some(inner),
            _ => None,
        }
    }

    /// Consumes the guard and returns the resource, if ready, otherwise returns self. **Does not poll**.
    pub fn into_ready(self) -> Result<B, Self> {
        match self {
            PendingResource::Ready { inner } => Ok(inner),
            other => Err(other),
        }
    }
}

mod private_async_src {
    use super::*;

    pub trait AsyncCheckProvider<T: ?Sized> {
        fn as_result_check(t: &T) -> MaResult<()>;
    }

    pub struct SourceCheckProvider;
    pub struct BufferCheckProvider;
    pub struct StreamCheckProvider;

    impl<F: PcmFormat, I> AsyncCheckProvider<ResourceManagerSource<F, I>> for SourceCheckProvider {
        fn as_result_check(t: &ResourceManagerSource<F, I>) -> MaResult<()> {
            resource_ffi::ma_resource_manager_data_source_result(t)
        }
    }

    impl<F: PcmFormat, I> AsyncCheckProvider<ResourceManagerBuffer<F, I>> for BufferCheckProvider {
        fn as_result_check(t: &ResourceManagerBuffer<F, I>) -> MaResult<()> {
            resource_ffi::ma_resource_manager_data_buffer_result(t)
        }
    }

    impl<F: PcmFormat, I> AsyncCheckProvider<ResourceManagerStream<F, I>> for StreamCheckProvider {
        fn as_result_check(t: &ResourceManagerStream<F, I>) -> MaResult<()> {
            resource_ffi::ma_resource_manager_data_stream_result(t)
        }
    }

    pub fn result_check<T: AsAsyncSource + ?Sized>(t: &T) -> MaResult<()> {
        <T as AsAsyncSource>::__ResultProvider::as_result_check(t)
    }
}

/// Unifies async result checks across sources.
pub trait AsAsyncSource: AsSourcePtr + SharedSource {
    type __ResultProvider: private_async_src::AsyncCheckProvider<Self>;
}

impl<F: PcmFormat, I> AsAsyncSource for ResourceManagerSource<F, I> {
    type __ResultProvider = private_async_src::SourceCheckProvider;
}
impl<F: PcmFormat, I> AsAsyncSource for ResourceManagerBuffer<F, I> {
    type __ResultProvider = private_async_src::BufferCheckProvider;
}
impl<F: PcmFormat, I> AsAsyncSource for ResourceManagerStream<F, I> {
    type __ResultProvider = private_async_src::StreamCheckProvider;
}

impl<F: PcmFormat> RmOps for ResourceManager<F> {}
impl<F: PcmFormat> RmOps for ResourceManagerRef<F> {}

/// Methods shared between [`ResourceManager`] and [`ResourceManagerRef`]
pub trait RmOps: AsRmPtr {
    fn load_name(&self, name: &str) -> MaResult<ResourceGuard<<Self as AsRmPtr>::Format, Unknown>> {
        resource_ffi::ma_resource_manager_register_encoded_data_internal(self, name, &[])?;
        let guard = ResourceGuard::<Self::Format, Unknown>::from_encoded_data(self, name, []);

        // If the name is not correct, miniaudio will assume this is a different source
        // Since we are not passing real data, this will error with INVALID_ARGS
        // if the name doesn't already exist in the binary tree
        let _ = guard.build_source(RmSourceFlags::ASYNC, None)?;

        Ok(guard)
    }

    fn load_path(
        &self,
        path: &Path,
    ) -> MaResult<ResourceGuard<<Self as AsRmPtr>::Format, Unknown>> {
        #[cfg(unix)]
        {
            let c_path = crate::cstring_from_path(path)?;
            resource_ffi::ma_resource_manager_register_encoded_data(
                self,
                c_path.as_ptr(),
                &[] as *const _,
                0,
            )?;
            let guard = ResourceGuard::<Self::Format, Unknown>::from_path(self, path);

            // If the name is not correct, miniaudio will assume this is a different source
            // Since we are not passing real data, this will error with INVALID_ARGS
            // if the name doesn't already exist in the binary tree
            let _ = guard.build_source(RmSourceFlags::ASYNC, None)?;

            Ok(guard)
        }

        #[cfg(windows)]
        {
            let name = crate::wide_null_terminated_name(name);
            resource_ffi::ma_resource_manager_register_encoded_data_w(
                rm,
                &name,
                &[] as *const _,
                0,
            )?;
            let guard = ResourceGuard::<Self::Format, Unknown>::from_path(self, path);

            // If the name is not correct, miniaudio will assume this is a different source
            // Since we are not passing real data, this will error with INVALID_ARGS
            // if the name doesn't already exist in the binary tree
            let _ = guard.build_source(RmSourceFlags::ASYNC, None)?;

            sOk(guard)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("only supported on unix and windows");
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_file(
        &self,
        path: &Path,
        flags: RmSourceFlags,
    ) -> MaResult<ResourceGuard<<Self as AsRmPtr>::Format, Unknown>> {
        #[cfg(unix)]
        {
            use crate::cstring_from_path;

            let c_path = cstring_from_path(path)?;
            resource_ffi::ma_resource_manager_register_file(self, c_path, flags)?;
            Ok(ResourceGuard::<Self::Format, Unknown>::from_path::<Self>(
                self, path,
            ))
        }

        #[cfg(windows)]
        {
            use crate::wide_null_terminated;

            let c_path = wide_null_terminated(path);

            resource_ffi::ma_resource_manager_register_file_w(self, &c_path, flags)?;
            Ok(ResourceGuard::<Self::Format>::from_path::<Self>(self, path))
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_decoded_u8(
        &self,
        name: &str,
        data: &[u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<Self::Format, u8>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<u8, Self>(
            self,
            name,
            data,
            Format::U8,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::<Self::Format, u8>::from_decoded_data::<Self>(self, name, data))
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_decoded_i16(
        &self,
        name: &str,
        data: &[i16],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<Self::Format, i16>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i16, Self>(
            self,
            name,
            data,
            Format::S16,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::<Self::Format, i16>::from_decoded_data::<Self>(self, name, data))
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_decoded_i32(
        &self,
        name: &str,
        data: &[i32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<Self::Format, i32>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<i32, Self>(
            self,
            name,
            data,
            Format::S32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::<Self::Format, i32>::from_decoded_data::<Self>(self, name, data))
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_decoded_s24_packed(
        &self,
        name: &str,
        data: &[u8],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<Self::Format, u8>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<S24Packed, Self>(
            self,
            name,
            data,
            Format::S24Packed,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::<Self::Format, u8>::from_decoded_data::<Self>(self, name, data))
    }

    /// The [`RmSourceFlags`] used are:
    /// - [`RmSourceFlags::WAIT_INIT`] -
    ///   Only meaningful with [`RmSourceFlags::ASYNC`]. When set, blocks until the
    ///   async initialization step has completed before returning.
    ///   This does not necessarily wait for the entire file to be decoded.
    /// - [`RmSourceFlags::ASYNC`] -
    ///   Perform loading/decoding work on the resource manager’s background threads.
    ///   Without this flag, registration performs the work synchronously.
    ///   (If the resource manager has threading disabled, this flag is ignored.)
    /// - [`RmSourceFlags::DECODE`] -
    ///   Decode and cache PCM during registration instead of deferring
    ///   decoding until the resource is initialized or read.
    fn register_decoded_f32(
        &self,
        name: &str,
        data: &[f32],
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<ResourceGuard<Self::Format, f32>> {
        resource_ffi::ma_resource_manager_register_decoded_data_internal::<f32, Self>(
            self,
            name,
            data,
            Format::F32,
            channels,
            sample_rate,
        )?;
        Ok(ResourceGuard::<Self::Format, f32>::from_decoded_data::<Self>(self, name, data))
    }

    /// Registers encoded/compressed audio bytes under a name.
    ///
    /// This only stores the provided bytes in the resource manager under `name`.
    /// No [`RmSourceFlags`] are applied at registration time because the underlying
    /// Miniaudio API does not accept flags for this operation.
    fn register_encoded(
        &self,
        name: &str,
        data: &[u8],
    ) -> MaResult<ResourceGuard<Self::Format, Unknown>> {
        resource_ffi::ma_resource_manager_register_encoded_data_internal(self, name, data)?;
        Ok(ResourceGuard::<Self::Format, Unknown>::from_encoded_data(
            self, name, data,
        ))
    }
}

impl<F: PcmFormat> ResourceManager<F> {
    fn new_with_config(config: &mut ResourceManagerBuilder<F>) -> MaResult<Self> {
        let (vtables, backend_reg) = if !config.vtables.is_empty() {
            let vtables = config.set_backend_vtables()?;
            let reg = config.set_backend_registration();
            (vtables, Some(reg))
        } else {
            (Box::default(), None)
        };

        let mut mem: Box<MaybeUninit<sys::ma_resource_manager>> = Box::new(MaybeUninit::uninit());

        resource_ffi::ma_resource_manager_init(config.as_raw_ptr(), mem.as_mut_ptr())?;

        let inner: *mut sys::ma_resource_manager =
            Box::into_raw(mem) as *mut sys::ma_resource_manager;

        Ok(Self(Arc::new(ResourceManagerInner {
            inner,
            channels: config.channels,
            format: config.format,
            backend_reg,
            _decoder_vtables: vtables,
            _format: PhantomData,
        })))
    }
}

pub(crate) mod resource_ffi {
    use std::path::Path;

    use crate::{
        audio::{
            channels::Channel,
            formats::{Format, SampleBuffer},
            sample_rate::SampleRate,
        },
        data_source::{DataFormat, DataSourceOps},
        engine::resource::{
            private_rm,
            rm_buffer::{ResourceManagerBuffer, ResourceManagerBufferBuilder},
            rm_source::{ResourceManagerSource, ResourceManagerSourceBuilder},
            rm_source_flags::RmSourceFlags,
            rm_stream::{ResourceManagerStream, ResourceManagerStreamBuilder},
            AsRmPtr, ResourceManager, ResourceManagerInner,
        },
        pcm_frames::PcmFormat,
        Binding, MaResult, MaudioError,
    };
    use crate::{AsRawRef, ErrorKinds};

    use maudio_sys::ffi as sys;

    #[inline]
    pub fn ma_resource_manager_init(
        config: *const sys::ma_resource_manager_config,
        rm: *mut sys::ma_resource_manager,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_init(config, rm) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_uninit<F: PcmFormat>(rm: &mut ResourceManagerInner<F>) {
        unsafe {
            sys::ma_resource_manager_uninit(rm.inner);
        }
    }

    // TODO: Implement Log
    #[inline]
    #[allow(dead_code)]
    pub fn ma_resource_manager_get_log<F: PcmFormat>(
        rm: &mut ResourceManager<F>,
    ) -> Option<*mut sys::ma_log> {
        let ptr = unsafe { sys::ma_resource_manager_get_log(rm.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    // REGISTRATION

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_register_file<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: std::ffi::CString,
        flags: RmSourceFlags,
    ) -> MaResult<()> {
        let res = unsafe {
            use crate::engine::resource::private_rm;
            sys::ma_resource_manager_register_file(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags.bits(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_register_file_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        path: &[u16],
        flags: RmSourceFlags,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_file_w(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags.bits(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    // Different from the miniaudio `static ma_result ma_resource_manager_register_decoded_data_internal`
    pub fn ma_resource_manager_register_decoded_data_internal<F: PcmFormat, R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
        data: &[F::StorageUnit],
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }

        let data_len = data.len();

        let units_per_frame = (channels as usize)
            .checked_mul(F::VEC_PCM_UNITS_PER_FRAME)
            .ok_or(MaudioError::new_ma_error(ErrorKinds::IntegerOverflow {
                op: "Units per frames",
            }))?;

        if data_len % units_per_frame != 0 {
            return Err(crate::MaudioError::new_ma_error(
                crate::ErrorKinds::InvalidDecodedDataLength,
            ));
        }

        let frame_count = (data_len / units_per_frame) as u64;

        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;
            ma_resource_manager_register_decoded_data(
                rm,
                name.as_ptr(),
                data.as_ptr() as *const _,
                frame_count,
                format,
                channels,
                sample_rate,
            )
        }
        #[cfg(windows)]
        {
            use crate::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            ma_resource_manager_register_decoded_data_w(
                rm,
                &name,
                data.as_ptr() as *const _,
                frame_count,
                format,
                channels,
                sample_rate,
            )
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_register_decoded_data<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        data: *const core::ffi::c_void,
        frame_count: u64,
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_decoded_data(
                private_rm::rm_ptr(rm),
                name,
                data,
                frame_count,
                format.into(),
                channels,
                sample_rate.into(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_register_decoded_data_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &[u16],
        data: *const core::ffi::c_void,
        frame_count: u64,
        format: Format,
        channels: u32,
        sample_rate: SampleRate,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_decoded_data_w(
                private_rm::rm_ptr(rm),
                name.as_ptr(),
                data,
                frame_count,
                format.into(),
                channels,
                sample_rate.into(),
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_register_encoded_data_internal<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &str,
        data: &[u8],
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;
            ma_resource_manager_register_encoded_data(
                rm,
                name.as_ptr(),
                data.as_ptr() as *const _,
                data.len(),
            )
        }
        #[cfg(windows)]
        {
            use crate::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            ma_resource_manager_register_encoded_data_w(
                rm,
                &name,
                data.as_ptr() as *const _,
                data.len(),
            )
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    pub fn ma_resource_manager_register_encoded_data<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data(private_rm::rm_ptr(rm), name, data, size)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    pub fn ma_resource_manager_register_encoded_data_w<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: &[u16],
        data: *const core::ffi::c_void,
        size: usize,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_register_encoded_data_w(
                private_rm::rm_ptr(rm),
                name.as_ptr(),
                data,
                size,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_unregister_file_internal(
        rm: *mut sys::ma_resource_manager,
        path: &Path,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let c_path = crate::cstring_from_path(path)?;
            ma_resource_manager_unregister_file(rm, c_path)
        }
        #[cfg(windows)]
        {
            let c_path = crate::wide_null_terminated(path);
            ma_resource_manager_unregister_file_w(rm, &c_path)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_unregister_file(
        rm: *mut sys::ma_resource_manager,
        path: std::ffi::CString,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_file(rm, path.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_unregister_file_w(
        rm: *mut sys::ma_resource_manager,
        path: &[u16],
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_file_w(rm, path.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_unregister_data_internal(
        rm: *mut sys::ma_resource_manager,
        name: &str,
    ) -> MaResult<()> {
        #[cfg(unix)]
        {
            let name = std::ffi::CString::new(name)
                .map_err(|_| crate::MaudioError::new_ma_error(crate::ErrorKinds::InvalidCString))?;

            ma_resource_manager_unregister_data(rm, name.as_ptr())
        }
        #[cfg(windows)]
        {
            use crate::wide_null_terminated_name;

            let name = wide_null_terminated_name(name);
            ma_resource_manager_unregister_data_w(rm, &name)
        }

        #[cfg(not(any(unix, windows)))]
        compile_error!("init decoder from file is only supported on unix and windows");
    }

    #[inline]
    #[cfg(unix)]
    fn ma_resource_manager_unregister_data(
        rm: *mut sys::ma_resource_manager,
        name: *const core::ffi::c_char,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_data(rm, name) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    fn ma_resource_manager_unregister_data_w(
        rm: *mut sys::ma_resource_manager,
        name: &[u16],
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_unregister_data_w(rm, name.as_ptr()) };
        MaudioError::check(res)?;
        Ok(())
    }

    // DATA BUFFERS

    #[inline]
    pub fn ma_resource_manager_data_buffer_init_ex<F: PcmFormat, I>(
        rm: *mut sys::ma_resource_manager,
        config: &ResourceManagerBufferBuilder<'_, F, I>,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_ex(rm, config.as_raw_ptr(), data_buffer)
        };
        MaudioError::check(res)
    }

    #[inline]
    #[cfg(unix)]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_init(
        rm: *mut sys::ma_resource_manager,
        path: std::ffi::CString,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init(rm, path.as_ptr(), flags, notif, data_buffer)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[cfg(windows)]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_init_w<F: PcmFormat>(
        rm: *mut sys::ma_resource_manager,
        path: &[u16],
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_w(
                rm,
                path.as_ptr(),
                flags,
                notif,
                data_buffer,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_init_copy<F: PcmFormat, I>(
        rm: *mut sys::ma_resource_manager,
        existing: &ResourceManagerBuffer<F, I>,
        data_buffer: *mut sys::ma_resource_manager_data_buffer,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_init_copy(rm, existing.to_raw(), data_buffer)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_uninit<F: PcmFormat, I>(
        data_buffer: &mut ResourceManagerBuffer<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_buffer_uninit(data_buffer.to_raw()) };
        MaudioError::check(res)?;
        Ok(())
    }

    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_read_pcm_frames<R: AsRmPtr, I>(
        data_buffer: &mut ResourceManagerBuffer<R::Format, I>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<R::Format>> {
        let channels = data_buffer.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<R::Format>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_buffer_read_pcm_frames_internal::<R, I>(
            data_buffer,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<R::Format>::from_storage(buffer, frames_read as usize, 2)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_read_pcm_frames_internal<R: AsRmPtr, I>(
        data_buffer: &mut ResourceManagerBuffer<R::Format, I>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_read_pcm_frames(
                data_buffer.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_seek_to_pcm_frame<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
        frame_index: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_seek_to_pcm_frame(
                data_buffer.to_raw(),
                frame_index,
            )
        };
        MaudioError::check(res)?;
        todo!()
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_get_data_format<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
        format: *mut sys::ma_format,
        channels: *mut u32,
        sample_rate: *mut u32,
        channel_map: *mut sys::ma_channel,
        channel_map_cap: usize,
    ) -> MaResult<Format> {
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_data_format(
                data_buffer.to_raw(),
                format,
                channels,
                sample_rate,
                channel_map,
                channel_map_cap,
            )
        };
        MaudioError::check(res)?;
        todo!()
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_get_cursor_in_pcm_frames<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
    ) -> MaResult<u64> {
        let mut cursor: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_cursor_in_pcm_frames(
                data_buffer.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_get_length_in_pcm_frames<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_length_in_pcm_frames(
                data_buffer.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_buffer_result<F: PcmFormat, I>(
        data_buffer: &ResourceManagerBuffer<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_buffer_result(data_buffer.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_set_looping<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_set_looping(data_buffer.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_is_looping<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_buffer_is_looping(data_buffer.to_raw()) };
        res == 1
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_buffer_get_available_frames<R: AsRmPtr, I>(
        data_buffer: &ResourceManagerBuffer<R::Format, I>,
    ) -> MaResult<u64> {
        let mut frames = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_buffer_get_available_frames(
                data_buffer.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // DATA SOURCES

    #[inline]
    pub fn ma_resource_manager_data_source_init_ex<F: PcmFormat, I>(
        rm: *mut sys::ma_resource_manager,
        config: &ResourceManagerSourceBuilder<'_, F, I>,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init_ex(rm, config.as_raw_ptr(), data_source)
        };
        MaudioError::check(res)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_init<R: AsRmPtr + ?Sized>(
        rm: &R,
        name: *const core::ffi::c_char,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init(
                private_rm::rm_ptr(rm),
                name,
                flags,
                notif,
                data_source,
            )
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_init_copy<F: PcmFormat, I>(
        rm: *mut sys::ma_resource_manager,
        existing: &ResourceManagerSource<F, I>,
        data_source: *mut sys::ma_resource_manager_data_source,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_init_copy(rm, existing.to_raw(), data_source)
        };
        MaudioError::check(res)?;
        Ok(())
    }

    #[inline]
    pub fn ma_resource_manager_data_source_uninit<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_source_uninit(data_source.to_raw()) };
        MaudioError::check(res)?;
        Ok(())
    }

    // Not used. Already available on DataSourceOps
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_read_pcm_frames<F: PcmFormat, I>(
        data_source: &mut ResourceManagerSource<F, I>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let channels = data_source.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_source_read_pcm_frames_internal::<F, I>(
            data_source,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, channels)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    fn ma_resource_manager_data_source_read_pcm_frames_internal<F: PcmFormat, I>(
        data_source: &mut ResourceManagerSource<F, I>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_read_pcm_frames(
                data_source.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_seek_to_pcm_frame<F: PcmFormat, I>(
        data_source: &mut ResourceManagerSource<F, I>,
        frame: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_source_seek_to_pcm_frame(data_source.to_raw(), frame)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_get_data_format<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_data_format(
                data_source.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map_raw.as_mut_ptr(),
                channel_map_raw.len(),
            )
        };
        MaudioError::check(res)?;
        // Could maybe cast when passing the ptr to miniaudio, but copying should be fine here
        let mut channel_map: Vec<Channel> = Vec::new();
        for c in channel_map_raw {
            channel_map.push(Channel::try_from(c)?);
        }
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map,
        })
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_get_cursor_in_pcm_frames<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_cursor_in_pcm_frames(
                data_source.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_get_length_in_pcm_frames<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_length_in_pcm_frames(
                data_source.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_source_result<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_source_result(data_source.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_set_looping<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_source_set_looping(data_source.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_is_looping<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_source_is_looping(data_source.to_raw()) };
        res == 1
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_source_get_available_frames<F: PcmFormat, I>(
        data_source: &ResourceManagerSource<F, I>,
    ) -> MaResult<u64> {
        let mut frames: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_source_get_available_frames(
                data_source.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // DATA STREAM

    #[inline]
    pub fn ma_resource_manager_data_stream_init_ex<F: PcmFormat, I>(
        rm: *mut sys::ma_resource_manager,
        config: &ResourceManagerStreamBuilder<'_, F, I>,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init_ex(rm, config.as_raw_ptr(), data_stream)
        };
        MaudioError::check(res)
    }

    #[cfg(unix)]
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_init<R: AsRmPtr>(
        rm: &R,
        path: std::ffi::CString,
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags,
                notif,
                data_stream,
            )
        };
        MaudioError::check(res)
    }

    #[cfg(windows)]
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_init_w<R: AsRmPtr>(
        rm: &R,
        path: &[u16],
        flags: u32,
        notif: *const sys::ma_resource_manager_pipeline_notifications,
        data_stream: *mut sys::ma_resource_manager_data_stream,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_init_w(
                private_rm::rm_ptr(rm),
                path.as_ptr(),
                flags,
                notif,
                data_stream,
            )
        };
        MaudioError::check(res)
    }

    #[inline]
    pub fn ma_resource_manager_data_stream_uninit<F: PcmFormat, I>(
        data_stream: &mut ResourceManagerStream<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_stream_uninit(data_stream.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_read_pcm_frames<F: PcmFormat, I>(
        data_stream: &mut ResourceManagerStream<F, I>,
        frame_count: u64,
    ) -> MaResult<SampleBuffer<F>> {
        let channels = data_stream.data_format()?.channels;
        if channels == 0 {
            return Err(MaudioError::from_ma_result(sys::ma_result_MA_INVALID_ARGS));
        }
        let mut buffer = SampleBuffer::<F>::new_zeroed(frame_count as usize, channels)?;

        let frames_read = ma_resource_manager_data_stream_read_pcm_frames_internal::<F, I>(
            data_stream,
            frame_count,
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
        )?;

        SampleBuffer::<F>::from_storage(buffer, frames_read as usize, channels)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    fn ma_resource_manager_data_stream_read_pcm_frames_internal<F: PcmFormat, I>(
        data_stream: &mut ResourceManagerStream<F, I>,
        frame_count: u64,
        buffer: *mut core::ffi::c_void,
    ) -> MaResult<u64> {
        let mut frames_read: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_read_pcm_frames(
                data_stream.to_raw(),
                buffer,
                frame_count,
                &mut frames_read,
            )
        };
        MaudioError::check(res)?;
        Ok(frames_read)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_seek_to_pcm_frame<F: PcmFormat, I>(
        data_stream: &mut ResourceManagerStream<F, I>,
        frame: u64,
    ) -> MaResult<()> {
        let res = unsafe {
            sys::ma_resource_manager_data_stream_seek_to_pcm_frame(data_stream.to_raw(), frame)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_get_data_format<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> MaResult<DataFormat> {
        let mut format_raw: sys::ma_format = sys::ma_format_ma_format_unknown;
        let mut channels: u32 = 0;
        let mut sample_rate: u32 = 0;
        let mut channel_map_raw = vec![0 as sys::ma_channel; sys::MA_MAX_CHANNELS as usize];
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_data_format(
                data_stream.to_raw(),
                &mut format_raw,
                &mut channels,
                &mut sample_rate,
                channel_map_raw.as_mut_ptr(),
                channel_map_raw.len(),
            )
        };
        MaudioError::check(res)?;
        // Could maybe cast when passing the ptr to miniaudio, but copying should be fine here
        let mut channel_map: Vec<Channel> = Vec::new();
        for c in channel_map_raw {
            channel_map.push(Channel::try_from(c)?);
        }
        channel_map.truncate(channels as usize);

        Ok(DataFormat {
            format: format_raw.try_into()?,
            channels,
            sample_rate: sample_rate.try_into()?,
            channel_map,
        })
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_get_cursor_in_pcm_frames<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> MaResult<u64> {
        let mut cursor = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_cursor_in_pcm_frames(
                data_stream.to_raw(),
                &mut cursor,
            )
        };
        MaudioError::check(res)?;
        Ok(cursor)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_get_length_in_pcm_frames<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> MaResult<u64> {
        let mut length: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_length_in_pcm_frames(
                data_stream.to_raw(),
                &mut length,
            )
        };
        MaudioError::check(res)?;
        Ok(length)
    }

    #[inline]
    pub fn ma_resource_manager_data_stream_result<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_data_stream_result(data_stream.to_raw()) };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_set_looping<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
        is_looping: bool,
    ) -> MaResult<()> {
        let is_looping = is_looping as u32;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_set_looping(data_stream.to_raw(), is_looping)
        };
        MaudioError::check(res)
    }

    // Not used. Already available on DataSourceOps
    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_is_looping<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> bool {
        let res = unsafe { sys::ma_resource_manager_data_stream_is_looping(data_stream.to_raw()) };
        res == 1
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_data_stream_get_available_frames<F: PcmFormat, I>(
        data_stream: &ResourceManagerStream<F, I>,
    ) -> MaResult<u64> {
        let mut frames: u64 = 0;
        let res = unsafe {
            sys::ma_resource_manager_data_stream_get_available_frames(
                data_stream.to_raw(),
                &mut frames,
            )
        };
        MaudioError::check(res)?;
        Ok(frames)
    }

    // JOB MANAGEMENT
    #[allow(unused)]
    pub fn ma_resource_manager_post_job<R: AsRmPtr>(
        rm: &R,
        job: *const sys::ma_job,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_post_job(private_rm::rm_ptr(rm), job) };
        MaudioError::check(res)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_post_job_quit<R: AsRmPtr>(rm: &R) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_post_job_quit(private_rm::rm_ptr(rm)) };
        MaudioError::check(res)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_next_job<R: AsRmPtr>(rm: &R, job: *mut sys::ma_job) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_next_job(private_rm::rm_ptr(rm), job) };
        MaudioError::check(res)
    }

    #[inline]
    #[allow(unused)]
    pub fn ma_resource_manager_process_next_job<R: AsRmPtr>(rm: &R) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_process_next_job(private_rm::rm_ptr(rm)) };
        MaudioError::check(res)
    }
}

impl<F: PcmFormat> Drop for ResourceManagerInner<F> {
    fn drop(&mut self) {
        resource_ffi::ma_resource_manager_uninit(self);
        for vtable in self._decoder_vtables.iter() {
            drop(unsafe { Box::from_raw(*vtable as *mut sys::ma_decoding_backend_vtable) });
        }
        if let Some(backend_reg) = self.backend_reg {
            drop(unsafe { Box::from_raw(backend_reg) });
        }
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[allow(dead_code)]
fn tiny_test_wav_mono(frames: usize) -> Vec<u8> {
    let mut samples = Vec::with_capacity(frames);
    for i in 0..frames {
        samples.push(((i as i32 * 300) % i16::MAX as i32) as i16);
    }
    wav_i16_le(1, SampleRate::Sr44100, &samples)
}

#[cfg(test)]
mod test {
    use crate::{
        engine::resource::{
            rm_builder::ResourceManagerBuilder, rm_source_flags::RmSourceFlags, tiny_test_wav_mono,
            RmOps,
        },
        test_assets::{
            decoded_data::{
                asset_interleaved_f32, asset_interleaved_i16, asset_interleaved_i32,
                asset_interleaved_s24_packed_le, asset_interleaved_u8,
            },
            temp_file::{unique_tmp_path, TempFileGuard},
        },
    };

    #[test]
    fn test_resource_man_basic_init() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        drop(rm);
    }

    #[test]
    fn test_resource_man_basic_init_2() {
        let rm = ResourceManagerBuilder::new_f32()
            .job_thread_count(1)
            .channels(2)
            .sample_rate(crate::audio::sample_rate::SampleRate::Sr11025)
            .build()
            .unwrap();
        drop(rm);
    }

    #[test]
    fn test_resource_man_basic_register_file() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let wav = tiny_test_wav_mono(20);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        std::fs::write(path_guard.path(), &wav).unwrap();
        let guard = rm
            .register_file(path_guard.path(), RmSourceFlags::NONE)
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
        let _strm = guard.build_stream(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_basic_register_encoded_data() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let wav: Vec<u8> = tiny_test_wav_mono(20);
        let guard = rm.register_encoded("test:wav", &wav).unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_u8() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let data = asset_interleaved_u8(2, 100, 1);
        let guard = rm
            .register_decoded_u8(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_i16() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let data = asset_interleaved_i16(2, 100, 1);
        let guard = rm
            .register_decoded_i16(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_i32() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let data = asset_interleaved_i32(2, 100, 1);
        let guard = rm
            .register_decoded_i32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_decoded_f32() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let data = asset_interleaved_f32(2, 100, 1.0);
        let guard = rm
            .register_decoded_f32(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        drop(buf);
        let src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
        drop(src);
    }

    #[test]
    fn test_resource_man_decoded_s24_packed() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();
        let data = asset_interleaved_s24_packed_le(2, 100, 1);
        let guard = rm
            .register_decoded_s24_packed(
                "data",
                &data,
                2,
                crate::audio::sample_rate::SampleRate::Sr48000,
            )
            .unwrap();
        let _buf = guard.build_buffer(RmSourceFlags::NONE, None).unwrap();
        let _src = guard.build_source(RmSourceFlags::NONE, None).unwrap();
    }

    #[test]
    fn test_resource_man_async_without_fence() {
        let rm = ResourceManagerBuilder::new_f32().build().unwrap();

        let wav = tiny_test_wav_mono(200);
        let path_guard = TempFileGuard::new(unique_tmp_path("wav"));
        let path = path_guard.path().to_path_buf();
        std::fs::write(&path, &wav).unwrap();

        let guard = rm.register_file(&path, RmSourceFlags::ASYNC).unwrap();
        let mut buffer = guard.build_buffer(RmSourceFlags::ASYNC, None).unwrap();
        let now = std::time::Instant::now();
        let mut elapsed: std::time::Duration;
        let mut loops: usize = 0;
        loop {
            elapsed = now.elapsed();
            if buffer.poll_ready().unwrap() {
                println!(
                    "Succesful poll after: {} micros and {loops} loops",
                    elapsed.as_micros()
                );
                break;
            }
            if elapsed.as_millis() > 20 {
                assert!(buffer.is_ready(), "Polling timed out, resource not ready");
                break;
            }
            loops += 1;
            std::thread::sleep(std::time::Duration::from_micros(5));
        }
    }
}
