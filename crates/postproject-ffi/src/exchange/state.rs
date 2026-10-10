//! Copied source-scoped inspection and continuation values.

use postproject_protocol::{Digest, FailureKind, Position, ProtocolBase, StoreRole};
use postproject_storage_sqlite::{ExchangeError, ExchangeResult};

use super::{PpHistoryId, PpProtocolBase, error};
use crate::{
    PpError, PpProduction, initialize_value, invalid_argument, lock_production, require_output,
};

/// Complete source replay boundary, including its original integrity digest.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpExchangePosition {
    /// Source decision, never rebound to the receiving file.
    pub base: PpProtocolBase,
    /// Exact 32-byte BLAKE3 commitment to this complete boundary.
    pub digest: [u8; 32],
}

/// Receiver-controlled positive file, private disk and decoding work budgets.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpExchangeLimits {
    /// Encoded file bytes, default 1 GiB.
    pub file_bytes: u64,
    /// Private checkpoint database plus journal bytes, default 2 GiB.
    pub disk_bytes: u64,
    /// Checkpoint frames and independent traversal work, default 10 million.
    pub frames: u64,
    /// Per-document encoded bytes, default 64 MiB.
    pub document_bytes: u64,
    /// Container depth from 1 through 192.
    pub document_depth: u32,
    /// Per-document JSON node budget, default 1 million.
    pub document_nodes: u64,
}

impl Default for PpExchangeLimits {
    fn default() -> Self {
        Self {
            file_bytes: 1 << 30,
            disk_bytes: 2 << 30,
            frames: 10_000_000,
            document_bytes: 64 << 20,
            document_depth: 192,
            document_nodes: 1_000_000,
        }
    }
}

/// One coherent copied authority/passive inspection, independent of its handle.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PpExchangeState {
    /// Current portable protocol major, independent of package and C ABI.
    pub protocol_version: u32,
    /// One authority, two passive mirror.
    pub role: u32,
    /// Bitset of `PP_EXCHANGE_*` supported codec features.
    pub supported_features: u64,
    /// Observed native head, including an incomplete development boundary.
    pub observed_base: PpProtocolBase,
    /// Oldest complete continuation anchor.
    pub floor: PpExchangePosition,
    /// One if complete head is available; zero identifies a history gap.
    pub has_head: u8,
    /// Complete current head when present; all zero otherwise.
    pub head: PpExchangePosition,
    /// SDK default receiver budgets; operations accept explicit overrides.
    pub limits: PpExchangeLimits,
}

impl From<ProtocolBase> for PpProtocolBase {
    fn from(base: ProtocolBase) -> Self {
        Self {
            decision: base.decision().into(),
            history_id: PpHistoryId {
                bytes: *base.scope().history().as_bytes(),
            },
        }
    }
}

impl From<Position> for PpExchangePosition {
    fn from(position: Position) -> Self {
        Self {
            base: position.decision_base().into(),
            digest: *position.digest().as_bytes(),
        }
    }
}

impl TryFrom<PpExchangePosition> for Position {
    type Error = ExchangeError;
    fn try_from(position: PpExchangePosition) -> Result<Self, Self::Error> {
        let base = ProtocolBase::try_from(position.base)?;
        Ok(Self::new(
            base.scope(),
            base.decision().revision_id(),
            base.decision().sequence(),
            Digest::from_bytes(position.digest),
        )?)
    }
}

impl PpExchangePosition {
    pub(super) fn empty() -> Self {
        Self {
            base: PpProtocolBase {
                decision: crate::PpDecisionBase {
                    production_id: crate::PpProductionId { bytes: [0; 16] },
                    has_revision: 0,
                    revision_id: crate::PpRevisionId { bytes: [0; 16] },
                    revision_sequence: 0,
                },
                history_id: PpHistoryId { bytes: [0; 16] },
            },
            digest: [0; 32],
        }
    }
}

impl PpExchangeState {
    fn empty() -> Self {
        Self {
            protocol_version: 0,
            role: 0,
            supported_features: 0,
            observed_base: PpExchangePosition::empty().base,
            floor: PpExchangePosition::empty(),
            has_head: 0,
            head: PpExchangePosition::empty(),
            limits: PpExchangeLimits {
                file_bytes: 0,
                disk_bytes: 0,
                frames: 0,
                document_bytes: 0,
                document_depth: 0,
                document_nodes: 0,
            },
        }
    }
}

pub(super) fn inspect(source: &crate::SqliteProduction) -> ExchangeResult<PpExchangeState> {
    let view = source.read_session()?;
    let decision = view.decision_base();
    let view = view.into_read_only();
    let head = match view.exchange_head() {
        Ok(head) => Some(head),
        Err(ExchangeError::Protocol(error)) if error.kind() == FailureKind::HistoryGap => None,
        Err(error) => return Err(error),
    };
    Ok(PpExchangeState {
        protocol_version: 1,
        role: match view.exchange_role() {
            StoreRole::Authority => 1,
            StoreRole::PassiveMirror => 2,
        },
        supported_features: 511,
        observed_base: ProtocolBase::new(view.exchange_scope()?, decision)?.into(),
        floor: view.exchange_floor()?.into(),
        has_head: u8::from(head.is_some()),
        head: head.map_or_else(PpExchangePosition::empty, Into::into),
        limits: PpExchangeLimits::default(),
    })
}

/// Copies coherent role, source scope, floor, available head and default budgets.
///
/// Missing complete capture sets `has_head=0`, preserving the native observed base.
/// Other corruption/storage errors keep their category; no recovery is implicit.
///
/// # Safety
/// Production live/serialized, output writable, error optional.
#[postproject_ffi_macros::ffi_export]
pub unsafe extern "C" fn pp_production_exchange_state(
    production: *const PpProduction,
    out_state: *mut PpExchangeState,
    out_error: *mut *mut PpError,
) -> u32 {
    // SAFETY: Clear copied output before validating the nullable owning handle.
    unsafe {
        initialize_value(out_state, PpExchangeState::empty());
        error::call(out_error, || {
            require_output(out_state, "out_state")?;
            let production = production
                .as_ref()
                .ok_or_else(|| invalid_argument("production must not be null"))?;
            out_state.write(inspect(&lock_production(&production.state))?);
            Ok(())
        })
    }
}
