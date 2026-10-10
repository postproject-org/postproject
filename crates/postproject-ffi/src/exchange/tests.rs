//! Native preparation preserves public intent and staging ownership boundaries.

use std::ptr;

use postproject_core::{
    DecisionBase, MediaRoot, MetadataProperty, MetadataValue, ObjectRef, PropertyId, VocabularyId,
};
use postproject_protocol::{Command, Document, HistoryId, Limits, OutcomeStatus};

use super::*;
use crate::{SqliteProduction, StagedMutation, begin_transaction_handle, production_handle};

fn make_root(name: &str, priority: i32) -> MediaRoot {
    MediaRoot::new(crate::MediaRootId::new(), name, None, None, priority, true).unwrap()
}

fn ids() -> (PpClientId, PpRequestId) {
    (
        PpClientId { bytes: [1; 16] },
        PpRequestId { bytes: [2; 16] },
    )
}

#[test]
fn preparation_preserves_order_exact_values_and_lost_reply_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let production = production_handle(SqliteProduction::create(&path, None).unwrap());
    let base = lock_production(&production.state)
        .read_session()
        .unwrap()
        .decision_base();
    let root = make_root("rushes", -5);
    let property = MetadataProperty::new(
        VocabularyId::new("unknown:Exact名").unwrap(),
        PropertyId::new("count").unwrap(),
    );
    let (client, request) = ids();
    // SAFETY: Fixture owns all handles and serialized writable outputs.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, Some(base)).unwrap();
        (*edit)
            .mutations
            .push(StagedMutation::MediaRoot(root.clone()));
        (*edit).mutations.push(StagedMutation::AddMetadataValue(
            ObjectRef::Production(base.production_id()),
            property.clone(),
            MetadataValue::u64(u64::MAX),
        ));
        let mut proposal = ptr::null_mut();
        assert_eq!(
            pp_transaction_prepare_proposal(
                edit,
                client,
                request,
                ptr::null(),
                &raw mut proposal,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        assert!((*edit).lifecycle.ensure_open().is_err());
        assert!(
            lock_production(&production.state)
                .latest_revision()
                .unwrap()
                .is_none()
        );
        assert_eq!(
            (*proposal).proposal.commands(),
            &[
                Command::AddMediaRoot(root.clone()),
                Command::AppendMetadata {
                    target: ObjectRef::Production(base.production_id()),
                    property,
                    value: MetadataValue::u64(u64::MAX)
                },
            ]
        );
        let mut bytes = ptr::null();
        let mut length = 0;
        assert_eq!(
            pp_proposal_document(proposal, &raw mut bytes, &raw mut length, ptr::null_mut()),
            crate::PP_OK
        );
        let document = Document::parse(
            std::slice::from_raw_parts(bytes, usize::try_from(length).unwrap()),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            Proposal::from_document(&document).unwrap(),
            (*proposal).proposal
        );
        let outcome = lock_production(&production.state)
            .submit_proposal(&(*proposal).proposal)
            .unwrap();
        assert!(
            matches!(outcome.status(), OutcomeStatus::Accepted(receipt) if receipt.revision().unwrap().sequence() == 1)
        );
        let next = begin_transaction_handle(&production.state, None, None).unwrap();
        crate::pp_transaction_release(edit);
        assert!(begin_transaction_handle(&production.state, None, None).is_err());
        crate::pp_transaction_release(next);
        let mut reopened = SqliteProduction::open(&path).unwrap();
        let mut later = reopened.begin_transaction().unwrap();
        later.add_media_root(make_root("later", 0)).unwrap();
        later.commit().unwrap();
        drop(later);
        assert_eq!(
            reopened.submit_proposal(&(*proposal).proposal).unwrap(),
            outcome
        );
        assert_eq!(reopened.latest_revision().unwrap().unwrap().sequence(), 2);
        pp_proposal_release(proposal);
    }
}

#[test]
fn rejected_preparation_keeps_edit_and_guard_with_cleared_output() {
    let directory = tempfile::tempdir().unwrap();
    let production = production_handle(
        SqliteProduction::create(directory.path().join("authority.pproj"), None).unwrap(),
    );
    let view = lock_production(&production.state).read_session().unwrap();
    let base = view.decision_base();
    let scope = view.into_read_only().exchange_scope().unwrap();
    let (client, request) = ids();
    // SAFETY: All pointers refer to live exclusively owned fixture values.
    unsafe {
        let edit = begin_transaction_handle(&production.state, None, Some(base)).unwrap();
        let mut proposal = ptr::dangling_mut();
        let foreign = PpProtocolBase {
            decision: base.into(),
            history_id: PpHistoryId {
                bytes: *HistoryId::new().as_bytes(),
            },
        };
        assert_eq!(
            pp_transaction_prepare_proposal(
                edit,
                client,
                request,
                &raw const foreign,
                &raw mut proposal,
                ptr::null_mut()
            ),
            14
        );
        assert!(proposal.is_null());
        (*edit).lifecycle.ensure_open().unwrap();
        assert!(begin_transaction_handle(&production.state, None, None).is_err());
        let mut wrong = PpProtocolBase {
            decision: base.into(),
            history_id: PpHistoryId {
                bytes: *scope.history().as_bytes(),
            },
        };
        wrong.decision = DecisionBase::new(base.production_id(), Some(crate::RevisionId::new()), 1)
            .unwrap()
            .into();
        assert_eq!(
            pp_transaction_prepare_proposal(
                edit,
                client,
                request,
                &raw const wrong,
                &raw mut proposal,
                ptr::null_mut()
            ),
            15
        );
        for _ in 0..=postproject_protocol::MAX_PROPOSAL_COMMANDS {
            (*edit)
                .mutations
                .push(StagedMutation::MediaRoot(make_root("bounded", 0)));
        }
        assert_eq!(
            pp_transaction_prepare_proposal(
                edit,
                client,
                request,
                ptr::null(),
                &raw mut proposal,
                ptr::null_mut()
            ),
            13
        );
        assert!(proposal.is_null());
        (*edit).lifecycle.ensure_open().unwrap();
        crate::pp_transaction_release(edit);
        let next = begin_transaction_handle(&production.state, None, None).unwrap();
        crate::pp_transaction_release(next);
        assert!(
            lock_production(&production.state)
                .latest_revision()
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn inspection_reports_incomplete_head_without_hiding_corruption() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("authority.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.add_media_root(make_root("rushes", 0)).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let production = production_handle(source);
    let mut output = std::mem::MaybeUninit::<PpExchangeState>::uninit();
    // SAFETY: Production remains live, output writable and inspected after OK.
    unsafe {
        assert_eq!(
            state::pp_production_exchange_state(
                &raw const production,
                output.as_mut_ptr(),
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        let mut state = output.assume_init();
        assert_eq!(state.role, 1);
        assert_eq!(state.head.base.decision.revision_sequence, 1);
        assert_eq!(state.has_head, 1);
        assert_eq!(state.floor.base.decision.revision_sequence, 0);
        assert_eq!(state.limits.document_bytes, 64 << 20);
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute("DELETE FROM exchange_records WHERE sequence = 1", [])
            .unwrap();
        assert_eq!(
            state::pp_production_exchange_state(
                &raw const production,
                &raw mut state,
                ptr::null_mut()
            ),
            crate::PP_OK
        );
        assert_eq!(state.has_head, 0);
        assert_eq!(state.observed_base.decision.revision_sequence, 1);
        assert_eq!(state.head.digest, [0; 32]);
        assert_eq!(state.head.base.decision.production_id.bytes, [0; 16]);
        connection
            .execute(
                "UPDATE exchange_history SET anchor_digest = zeroblob(32)",
                [],
            )
            .unwrap();
        assert_eq!(
            state::pp_production_exchange_state(
                &raw const production,
                &raw mut state,
                ptr::null_mut()
            ),
            crate::PP_ERROR_STORAGE
        );
        assert_eq!(state.protocol_version, 0);
        assert_eq!(state.limits.file_bytes, 0);
        assert_eq!(state.observed_base.decision.revision_sequence, 0);
    }
}

#[test]
fn proposal_parser_bounds_lengths_before_reading_and_clears_failed_outputs() {
    let mut output = ptr::dangling_mut();
    // SAFETY: Oversized input is rejected before dereferencing the deliberately
    // unreadable pointer; all accepted-length fixtures are live byte slices.
    unsafe {
        assert_eq!(
            pp_proposal_parse(ptr::dangling(), u64::MAX, &raw mut output, ptr::null_mut()),
            13
        );
        assert!(output.is_null());
        for bytes in [
            b"{}".as_slice(),
            b"{\"kind\":\"proposal\",\"kind\":\"proposal\"}".as_slice(),
            &[0xff],
        ] {
            assert_eq!(
                pp_proposal_parse(
                    bytes.as_ptr(),
                    u64::try_from(bytes.len()).unwrap(),
                    &raw mut output,
                    ptr::null_mut()
                ),
                12
            );
            assert!(output.is_null());
        }
        assert_eq!(
            pp_proposal_parse(ptr::null(), 0, &raw mut output, ptr::null_mut()),
            crate::PP_ERROR_INVALID_ARGUMENT
        );
        assert!(output.is_null());
    }
}
