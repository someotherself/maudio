use maudio_sys::ffi as sys;

use crate::AsRawRef;

struct NodeBuilder {
    inner: sys::ma_node_config,
}

impl AsRawRef for NodeBuilder {
    type Raw = sys::ma_node_config;

    fn as_raw(&self) -> &Self::Raw {
        &self.inner
    }
}
