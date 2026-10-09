//! Handled observation failures preserve the previously recorded evidence.

use postproject_core::{RepresentationFingerprint, ResourceFingerprint};
use postproject_media::prepare_original_media;
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::{Connection, params};

#[test]
fn failed_resource_invalidation_rolls_back_current_and_archived_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let media = directory.path().join("clip.mov");
    std::fs::write(&media, b"source bytes").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let resource = &import.resources()[0];
    let initial = &resource.fingerprints()[0];
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_marker BEFORE INSERT ON representation_fingerprint_recomputations BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    let changed =
        ResourceFingerprint::new(initial.algorithm(), initial.version(), vec![99]).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_resource_fingerprint(resource.id(), &changed)
            .is_err()
    );
    assert!(edit.commit().unwrap().revision().is_none());
    drop(edit);
    assert_eq!(
        source.resources(import.representation().id()).unwrap()[0].fingerprints(),
        resource.fingerprints()
    );
    assert_empty_table(&connection, "resource_fingerprint_history");
    assert_empty_table(&connection, "representation_fingerprint_recomputations");
    assert_eq!(source.latest_revision().unwrap().unwrap().sequence(), 1);
    connection
        .execute_batch("DROP TRIGGER reject_marker")
        .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_resource_fingerprint(resource.id(), &changed)
            .unwrap()
    );
    assert_eq!(edit.commit().unwrap().revision().unwrap().sequence(), 2);
}

#[test]
fn failed_marker_clear_preserves_representation_value_and_dirty_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let media = directory.path().join("clip.mov");
    std::fs::write(&media, b"source bytes").unwrap();
    let import = prepare_original_media(&media, None, None).unwrap();
    let representation = import.representation();
    let initial = &representation.fingerprints()[0];
    let resource = &import.resources()[0];
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(&import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let observed = &resource.fingerprints()[0];
    let changed =
        ResourceFingerprint::new(observed.algorithm(), observed.version(), vec![99]).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.record_resource_fingerprint(resource.id(), &changed)
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_marker_clear BEFORE DELETE ON representation_fingerprint_recomputations BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    let replacement =
        RepresentationFingerprint::new(initial.algorithm(), initial.version(), vec![88]).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_representation_fingerprint(representation.id(), &replacement)
            .is_err()
    );
    assert!(edit.commit().unwrap().revision().is_none());
    drop(edit);
    assert_eq!(
        source
            .representation(representation.id())
            .unwrap()
            .fingerprints(),
        representation.fingerprints()
    );
    assert_empty_table(&connection, "representation_fingerprint_history");
    let marker: (Vec<u8>, i64) = connection.query_row("SELECT changed_resource_id, marked_revision_sequence FROM representation_fingerprint_recomputations WHERE representation_id = ?1", params![representation.id().as_bytes().as_slice()], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(marker, (resource.id().as_bytes().to_vec(), 2));
    assert_eq!(source.latest_revision().unwrap().unwrap().sequence(), 2);
    connection
        .execute_batch("DROP TRIGGER reject_marker_clear")
        .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    assert!(
        edit.record_representation_fingerprint(representation.id(), &replacement)
            .unwrap()
    );
    assert_eq!(edit.commit().unwrap().revision().unwrap().sequence(), 3);
}

fn assert_empty_table(connection: &Connection, table: &str) {
    let count: i64 = connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0, "unexpected rows in {table}");
}
