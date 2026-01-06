use maudio_sys::ffi as sys;

pub struct NodeGraphConfig {
    inner: sys::ma_node_graph_config,
}

impl NodeGraphConfig {
    #[inline]
    pub(crate) fn get_raw(&self) -> *const sys::ma_node_graph_config {
        &self.inner as *const _
    }
}
