use maudio_sys::ffi as sys;

use crate::Binding;

pub struct ResourceManager {
    inner: *mut sys::ma_resource_manager,
}

impl Binding for ResourceManager {
    type Raw = *mut sys::ma_resource_manager;

    /// !!! unimplemented !!!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

pub(crate) mod resource_ffi {
    use maudio_sys::ffi as sys;

    use crate::{engine::resource::ResourceManager, Binding, MaResult, MaudioError};

    pub fn ma_resource_manager_init(
        config: *const sys::ma_resource_manager_config,
        rm: *mut sys::ma_resource_manager,
    ) -> MaResult<()> {
        let res = unsafe { sys::ma_resource_manager_init(config, rm) };
        MaudioError::check(res)?;
        Ok(())
    }

    pub fn ma_resource_manager_uninit(rm: &mut ResourceManager) {
        unsafe {
            sys::ma_resource_manager_uninit(rm.to_raw());
        }
    }

    // TODO: Implement Log
    pub fn ma_resource_manager_get_log(rm: &mut ResourceManager) -> Option<*mut sys::ma_log> {
        let ptr = unsafe { sys::ma_resource_manager_get_log(rm.to_raw()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
}
