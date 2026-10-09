use postproject_core::{FingerprintSnapshot, ObjectRef, ResourceId};
use postproject_protocol::{CheckpointSection, FingerprintObservation, FingerprintState};

use super::super::Bodies;
use crate::SqliteProduction;

mod support;
use support::{exported_documents, fixture, populate, portable_rows};

#[test]
fn fingerprint_transport_preserves_both_domains_intermediate_history_and_dirty_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let import = fixture();
    populate(&mut source, &import);
    let mut destination =
        SqliteProduction::create(directory.path().join("destination.pproj"), None).unwrap();
    let manifest = destination.export_checkpoint(|_| Ok(())).unwrap();
    let transaction = destination.connection.transaction().unwrap();
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .unwrap();
    let mut bodies = Bodies::new(&transaction, &manifest, crate::CheckpointLimits::default());
    let documents = exported_documents(&source);
    assert_eq!(documents.len(), 8); // Two current, five historical, one marker.
    for document in &documents {
        bodies.fingerprint(document).unwrap();
    }
    for table in [
        "resource_fingerprints",
        "representation_fingerprints",
        "resource_fingerprint_history",
        "representation_fingerprint_history",
        "representation_fingerprint_recomputations",
    ] {
        assert_eq!(
            portable_rows(&source.connection, table),
            portable_rows(&transaction, table)
        );
    }
    assert!(
        matches!(bodies.document(CheckpointSection::Fingerprints, &documents[0]), Err(crate::ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity)
    );
    drop(bodies);
    transaction.rollback().unwrap();
    assert_eq!(
        portable_rows(&destination.connection, "resource_fingerprints"),
        Vec::<Vec<rusqlite::types::Value>>::new()
    );
}

#[test]
fn duplicate_current_facts_and_noncontiguous_history_reject_before_publication() {
    let directory = tempfile::tempdir().unwrap();
    let mut destination =
        SqliteProduction::create(directory.path().join("destination.pproj"), None).unwrap();
    let manifest = destination.export_checkpoint(|_| Ok(())).unwrap();
    let transaction = destination.connection.transaction().unwrap();
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .unwrap();
    let mut bodies = Bodies::new(&transaction, &manifest, crate::CheckpointLimits::default());
    let target = ObjectRef::Resource(ResourceId::new());
    let snapshot = FingerprintSnapshot::new("unknown_CASE", 0, vec![0, 255], None).unwrap();
    let current = FingerprintObservation::new(target, snapshot.clone(), FingerprintState::Current)
        .unwrap()
        .document()
        .unwrap();
    bodies.fingerprint(&current).unwrap();
    assert!(
        matches!(bodies.fingerprint(&current), Err(crate::ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity)
    );
    let skipped = FingerprintObservation::new(
        target,
        snapshot.clone(),
        FingerprintState::Superseded {
            position: 1,
            revision_sequence: 2,
        },
    )
    .unwrap()
    .document()
    .unwrap();
    assert!(bodies.fingerprint(&skipped).is_err());
    drop(bodies);
    let mut bodies = Bodies::new(&transaction, &manifest, crate::CheckpointLimits::default());
    let first = FingerprintObservation::new(
        target,
        snapshot,
        FingerprintState::Superseded {
            position: 0,
            revision_sequence: 2,
        },
    )
    .unwrap()
    .document()
    .unwrap();
    bodies.fingerprint(&first).unwrap();
    assert!(bodies.fingerprint(&first).is_err());
    drop(bodies);
    transaction.rollback().unwrap();
}

#[test]
fn fingerprint_consistency_checks_reject_changed_boundaries_and_broken_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    populate(&mut source, &fixture());
    super::validate::boundaries(&source.connection, 2).unwrap();
    for sql in [
        "UPDATE resource_fingerprints SET observed_revision_sequence = 3",
        "UPDATE resource_fingerprint_history SET superseded_revision_sequence = 3",
        "UPDATE resource_fingerprint_history SET observed_revision_sequence = 1 WHERE id = (SELECT max(id) FROM resource_fingerprint_history)",
        "UPDATE resource_fingerprint_history SET value = x'02' WHERE id = (SELECT max(id) FROM resource_fingerprint_history)",
        "UPDATE resource_fingerprints SET value = x'03'",
        "DELETE FROM representation_fingerprints",
        "UPDATE representation_fingerprint_recomputations SET marked_revision_sequence = 3",
    ] {
        source
            .connection
            .execute_batch("SAVEPOINT invalid_evidence")
            .unwrap();
        source.connection.execute(sql, []).unwrap();
        assert!(
            matches!(super::validate::boundaries(&source.connection, 2), Err(crate::ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity),
            "{sql}"
        );
        source
            .connection
            .execute_batch("ROLLBACK TO invalid_evidence; RELEASE invalid_evidence")
            .unwrap();
    }
    super::validate::boundaries(&source.connection, 2).unwrap();
}
