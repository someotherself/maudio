use maudio_sys::ffi as sys;

pub type NodeFlagsRaw = sys::ma_node_flags;

/// Flags that control how a node behaves inside the node graph.
///
/// These flags influence when the node's processing callback is invoked,
/// how missing input is handled, and how the node contributes to the final mix.
#[repr(transparent)]
#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
struct NodeFlags(NodeFlagsRaw);

impl NodeFlags {
    pub const NONE: Self = Self(0);
    /// Marks the node as a transparent pass-through.
    ///
    /// The node performs no audio processing and simply copies audio from
    /// its input bus directly to its output bus.
    ///
    /// # Requirements
    /// - Exactly **1 input bus** and **1 output bus**
    /// - Input and output must have the **same channel count**
    ///
    /// # Typical uses
    /// - Time or position tracking
    /// - Event scheduling
    /// - Marker or control nodes
    pub const PASSTHROUGH: Self = Self(sys::ma_node_flags_MA_NODE_FLAG_PASSTHROUGH);

    /// Forces the node's processing callback to be invoked continuously.
    ///
    /// By default, if a node has input busses but no inputs are attached
    /// (or upstream nodes have finished producing audio), the processing
    /// callback will **not** be called.
    ///
    /// When this flag is set, the processing callback is invoked regardless
    /// of whether any input audio is available.
    ///
    /// # Typical uses
    /// - Effects with tails (reverb, delay, echo)
    /// - Nodes that generate output independently of inputs
    /// - Nodes that must advance internal state every frame
    ///
    /// This flag is required for nodes that must "keep running" after their
    /// inputs go silent.
    pub const CONTINUOUS_PROCESSING: Self =
        Self(sys::ma_node_flags_MA_NODE_FLAG_CONTINUOUS_PROCESSING);

    /// Allows the processing callback to receive `NULL` input buffers.
    ///
    /// This flag is used in conjunction with [`CONTINUOUS_PROCESSING`].
    ///
    /// ## Behavior
    /// - **When set**: `ppFramesIn` (TODO) will be `NULL` (TODO) when no input data is available
    /// - **When unset**: silence is supplied instead
    ///
    /// # Typical uses
    /// - Procedural generators
    /// - Nodes that explicitly distinguish "no input" from "silent input"
    /// - Advanced DSP where silence is not equivalent to absence of data
    ///
    /// If your node treats silence and missing input the same way, this
    /// flag is usually unnecessary.
    pub const ALLOW_NULL_INPUT: Self = Self(sys::ma_node_flags_MA_NODE_FLAG_ALLOW_NULL_INPUT);

    /// Indicates that the node processes input and output frames at
    /// different rates.
    ///
    /// This tells miniaudio that the number of input frames consumed may
    /// differ from the number of output frames produced.
    ///
    /// # Required for
    /// - Resampling nodes
    /// - Time-stretching or pitch-shifting nodes
    /// - Any node that changes playback rate
    ///
    /// Omitting this flag for such nodes will result in incorrect graph
    /// scheduling and audio glitches.
    pub const DIFFERENT_PROCESSING_RATES: Self =
        Self(sys::ma_node_flags_MA_NODE_FLAG_DIFFERENT_PROCESSING_RATES);

    /// Marks the node as producing silent output.
    ///
    /// The node's output does not contribute to the final mix, even if
    /// audio is written to the output buffer.
    ///
    /// # Typical uses
    /// - Analysis or metering nodes
    /// - Recording or file-writing branches
    /// - Visualization or monitoring nodes
    ///
    /// When this flag is set, writing to the output buffer is unnecessary,
    /// as miniaudio will ignore it.
    pub const SILENT_OUTPUT: Self = Self(sys::ma_node_flags_MA_NODE_FLAG_SILENT_OUTPUT);

    #[inline]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::useless_conversion)]
    #[allow(clippy::unnecessary_cast)]
    pub fn bits(self) -> u32 {
        self.0 as u32
    }

    /// Set or clear bits
    #[inline]
    pub const fn set(&mut self, other: Self, enabled: bool) {
        if enabled {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    /// Create NodeFlags from a u32 bitmask
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits as NodeFlagsRaw)
    }

    /// Check if all the bits in other are set
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if any of the bits in other are set
    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn insert(&mut self, other: Self) {
        self.0 |= other.0
    }

    #[inline]
    pub(crate) const fn insert_bits(&mut self, other: &Self) {
        self.0 |= other.0
    }

    #[inline]
    pub const fn remove(&mut self, other: Self) {
        self.0 &= !other.0
    }
}

impl core::ops::BitOr for NodeFlags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl core::ops::BitOrAssign for NodeFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl core::ops::BitAnd for NodeFlags {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}
impl core::ops::BitAndAssign for NodeFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
impl core::ops::BitXor for NodeFlags {
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}
impl core::ops::Not for NodeFlags {
    type Output = Self;
    #[inline]
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl From<NodeFlags> for u32 {
    #[inline]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::useless_conversion)]
    #[allow(clippy::unnecessary_cast)]
    fn from(v: NodeFlags) -> u32 {
        v.0 as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_node_flags() {
        let mut flag = NodeFlags::NONE;
        flag.insert(NodeFlags::PASSTHROUGH);
        assert!(flag == NodeFlags::PASSTHROUGH);
        flag.remove(NodeFlags::PASSTHROUGH);
        assert!(flag == NodeFlags::NONE);

        flag.insert(NodeFlags::PASSTHROUGH);
        flag.insert(NodeFlags::CONTINUOUS_PROCESSING);
        assert!(flag == (NodeFlags::PASSTHROUGH | NodeFlags::CONTINUOUS_PROCESSING));
        assert!(flag.contains(NodeFlags::PASSTHROUGH));
        assert!(flag.contains(NodeFlags::CONTINUOUS_PROCESSING));
        assert!(!flag.contains(NodeFlags::SILENT_OUTPUT));

        flag.remove(NodeFlags::PASSTHROUGH);
        assert!(flag == NodeFlags::CONTINUOUS_PROCESSING);
    }
}
