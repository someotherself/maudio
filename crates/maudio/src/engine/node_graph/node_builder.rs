use maudio_sys::ffi as sys;

use crate::{Binding, ErrorKinds, MaudioError};

struct NodeBuilder {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    type Error = MaudioError;

    fn try_from(value: sys::ma_pan_mode) -> Result<Self, Self::Error> {
        match value {
            sys::ma_node_state_ma_node_state_started => Ok(NodeState::Started),
            sys::ma_node_state_ma_node_state_stopped => Ok(NodeState::Stopped),
            _ => Err(MaudioError::new_ma_error(ErrorKinds::InvalidNodeState)),
        }
    }
}
