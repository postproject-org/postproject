//! Native projection of checked public exchange values and ownership.

/// Persistent source-history identity, distinct from a production ID.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpHistoryId {
    /// Stable UUID bytes; existence is checked by exchange operations.
    pub bytes: [u8; 16],
}

/// Stable submitting client identity; create once and retain across retries.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpClientId {
    /// Stable UUID bytes, without authentication semantics.
    pub bytes: [u8; 16],
}

/// Stable identity of one complete intent; reuse after an uncertain response.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PpRequestId {
    /// Stable UUID bytes; a retained identity must keep its original intent.
    pub bytes: [u8; 16],
}

/// Detached domain decision retaining its original source history.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpProtocolBase {
    /// Production and checked revision/sequence of the observed decision.
    pub decision: crate::PpDecisionBase,
    /// Source history; never replace this with the destination's history.
    pub history_id: PpHistoryId,
}
