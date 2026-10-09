use postproject_core::{FingerprintSnapshot, RepresentationFingerprint};
use postproject_protocol::{ActivityHeader, Document, encode_fingerprint_snapshot};

use crate::{
    SqliteProduction,
    exchange::{activity_capture, records::ActivityApply},
};

#[test]
fn current_activity_staging_keeps_snapshots_after_their_inputs_changed() {
    let directory = tempfile::tempdir().unwrap();
    let (source, activity, imports) = super::source(directory.path());
    let mut destination =
        SqliteProduction::create(directory.path().join("staged.pproj"), None).unwrap();
    let base = destination.read_session().unwrap().decision_base();
    let mut edit = destination.begin_edit(base).unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.record_representation_fingerprint(
        imports[0].representation().id(),
        &RepresentationFingerprint::new("unknown_CASE", 1, vec![99]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let frames = frames(&source, activity.id());
    let transaction = destination.connection.transaction().unwrap();
    let mut pending = ActivityApply::begin_checkpoint(
        &transaction,
        ActivityHeader::from_document(&frames[0]).unwrap(),
    )
    .unwrap();
    for (index, frame) in frames[1..].iter().enumerate() {
        assert_eq!(
            pending.push_checkpoint(&transaction, 1, frame).unwrap(),
            index == frames.len() - 2
        );
    }
    transaction.commit().unwrap();
    assert_eq!(
        source.activities().unwrap(),
        destination.activities().unwrap()
    );
    drop(destination);
    let reopened = SqliteProduction::open(directory.path().join("staged.pproj")).unwrap();
    assert_eq!(source.activities().unwrap(), reopened.activities().unwrap());
}

#[test]
fn snapshot_after_its_edge_boundary_rejects_even_before_the_checkpoint_head() {
    let directory = tempfile::tempdir().unwrap();
    let (source, activity, _) = super::source(directory.path());
    let transaction = source.connection.unchecked_transaction().unwrap();
    // A different activity avoids duplicate scalar identity while reusing valid references.
    let header = ActivityHeader::new(
        postproject_core::ActivityId::new(),
        activity.kind().clone(),
        1,
        1,
    )
    .unwrap();
    let mut pending = ActivityApply::begin_checkpoint(&transaction, header.clone()).unwrap();
    let edge = postproject_protocol::ActivityEdgeHeader::new(
        header.id(),
        postproject_protocol::ActivityEdgeSide::Input,
        0,
        activity.inputs()[0].representation_id(),
        None,
    )
    .unwrap()
    .with_snapshot(1, 1)
    .unwrap()
    .with_dependency_snapshot(0)
    .unwrap();
    assert!(
        !pending
            .push_checkpoint(&transaction, 3, &edge.document())
            .unwrap()
    );
    let future = FingerprintSnapshot::new("unknown_CASE", 1, vec![1], Some(2)).unwrap();
    assert!(
        pending
            .push_checkpoint(
                &transaction,
                3,
                &encode_fingerprint_snapshot(&future).unwrap()
            )
            .is_err()
    );
    transaction.rollback().unwrap();
    assert_eq!(source.activities().unwrap().len(), 1);
}

fn frames(source: &SqliteProduction, id: postproject_core::ActivityId) -> Vec<Document> {
    let mut frames = Vec::new();
    activity_capture::write(
        &source.connection,
        &super::super::header(&source.connection, id).unwrap(),
        &mut |frame| -> postproject_core::Result<()> {
            frames.push(frame.clone());
            Ok(())
        },
    )
    .unwrap();
    frames
}
