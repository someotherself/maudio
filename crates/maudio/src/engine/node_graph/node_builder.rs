use maudio_sys::ffi as sys;

use crate::{Binding, MaError};

pub struct NodeBuilder {
    inner: sys::ma_node_config,
}

impl Binding for NodeBuilder {
    type Raw = sys::ma_node_config;

    // !! unimplemented !!
    fn from_ptr(_raw: Self::Raw) -> Self {
        unimplemented!()
    }

    fn to_raw(&self) -> Self::Raw {
        self.inner
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub enum NodeState {
    Started,
    Stopped,
}

impl From<NodeState> for sys::ma_node_state {
    fn from(value: NodeState) -> Self {
        match value {
            NodeState::Started => sys::ma_node_state_ma_node_state_started,
            NodeState::Stopped => sys::ma_node_state_ma_node_state_stopped,
        }
    }
}

impl TryFrom<sys::ma_node_state> for NodeState {
    type Error = MaError;

    fn try_from(value: sys::ma_pan_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_node_state_ma_node_state_started => Ok(NodeState::Started),
            sys::ma_node_state_ma_node_state_stopped => Ok(NodeState::Stopped),
            _ => Err(MaError(sys::ma_result_MA_INVALID_ARGS)),
        }
    }
}

// impl NodeBuilder {
//     pub fn new() -> Self {
//         let inner = unsafe { sys::ma_node_config_init() };
//         Self {
//             inner,
//             in_bus_channels: None,
//             out_bus_channels: None,
//             in_bus_count: None,
//             out_bus_count: None,
//             initial_state: NodeState::Started
//         }
//     }

//     pub fn node_type(mut self, node_type: NodeType) -> Self {
//         self.inner.vtable = node_type.as_ptr();
//         self
//     }

//     pub fn in_buses(mut self) -> Self {
//         todo!()
//     }

//     pub fn out_buses(mut self) -> Self {
//         todo!()
//     }
//     pub fn channels_in(mut self, channels: u32) -> Self {
//         todo!()
//     }

//     pub fn channels_out(mut self, channels: u32) -> Self {
//         todo!()
//     }

//     pub fn vtable(mut self) -> Self {
//         todo!()
//     }

//     pub fn bus_count(mut self) -> Self {
//         todo!()
//     }

//     pub fn initial_state(mut self, state: NodeState) -> Self {
//         self.inner.initialState = state as u32;
//         self.initial_state = state;
//         self
//     }
// }

// pub struct NodeType {
//     inner: &'static sys::ma_node_vtable
// }

// impl NodeType {
//     pub(crate) const fn new(inner: &'static sys::ma_node_vtable) -> Self {
//         Self { inner }
//     }
//     #[inline]
//     fn as_ptr(self) -> *const sys::ma_node_vtable {
//         self.inner as *const _
//     }
// }
