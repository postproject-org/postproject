use postproject_core::{FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{FingerprintChangeStart, FingerprintRecomputation};
use rusqlite::{Connection, params};

#[test]
fn resource_marking_and_unchanged_representation_clearing_preserve_exact_evidence() {
    let connection = fixture();
    super::born(&connection, representation()).unwrap();
    assert!(super::resource_start(&connection, resource(), 0).is_err());
    super::resource_start(&connection, resource(), 1).unwrap();
    assert!(super::resource_finish(&connection, 1).is_err());
    let dirty = marker(2);
    super::marked(&connection, dirty, resource(), 2).unwrap();
    assert!(super::marked(&connection, dirty, resource(), 2).is_err());
    super::resource_finish(&connection, 1).unwrap();
    assert!(super::finish(&connection, 0).is_err());
    insert_current(&connection, 2);
    let wrong = clearing(marker(1));
    assert!(super::cleared(&connection, &wrong, 2, 0).is_err());
    super::cleared(&connection, &clearing(dirty), 2, 0).unwrap();
    assert!(super::finish(&connection, 0).is_err());
    connection
        .execute("DELETE FROM representation_fingerprint_recomputations", [])
        .unwrap();
    super::finish(&connection, 0).unwrap();
}

#[test]
fn foreign_resources_owners_and_revision_boundaries_cannot_create_dirty_markers() {
    let connection = fixture();
    super::resource_start(&connection, resource(), 1).unwrap();
    assert!(super::marked(&connection, marker(2), ResourceId::new(), 2).is_err());
    assert!(super::marked(&connection, marker(2), resource(), 3).is_err());
    let foreign = FingerprintRecomputation::new(RepresentationId::new(), resource(), 2).unwrap();
    assert!(super::marked(&connection, foreign, resource(), 2).is_err());
    assert!(super::resource_finish(&connection, 1).is_err());
    super::marked(&connection, marker(2), resource(), 2).unwrap();
    super::resource_finish(&connection, 1).unwrap();
}

#[test]
fn unknown_baseline_markers_are_allowed_but_post_floor_evidence_needs_an_authored_transition() {
    let connection = fixture();
    insert_current(&connection, 1);
    assert!(super::finish(&connection, 0).is_err());
    super::finish(&connection, 1).unwrap();
    super::create(&connection).unwrap();
    assert!(super::cleared(&connection, &clearing(marker(2)), 2, 1).is_err());
    super::cleared(&connection, &clearing(marker(1)), 2, 1).unwrap();
    connection
        .execute("DELETE FROM representation_fingerprint_recomputations", [])
        .unwrap();
    super::finish(&connection, 1).unwrap();
}

fn fixture() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE representation_resources (representation_id BLOB, resource_id BLOB); CREATE TABLE representation_fingerprint_recomputations (representation_id BLOB PRIMARY KEY, changed_resource_id BLOB NOT NULL, marked_revision_sequence INTEGER NOT NULL);").unwrap();
    connection
        .execute(
            "INSERT INTO representation_resources VALUES (?1, ?2)",
            params![
                representation().as_bytes().as_slice(),
                resource().as_bytes().as_slice()
            ],
        )
        .unwrap();
    super::create(&connection).unwrap();
    connection
}

fn representation() -> RepresentationId {
    RepresentationId::from_bytes([2; 16])
}
fn resource() -> ResourceId {
    ResourceId::from_bytes([1; 16])
}
fn marker(sequence: u64) -> FingerprintRecomputation {
    FingerprintRecomputation::new(representation(), resource(), sequence).unwrap()
}

fn clearing(marker: FingerprintRecomputation) -> FingerprintChangeStart {
    let snapshot = FingerprintSnapshot::new("Exact_CASE", 0, vec![0, 255], Some(1)).unwrap();
    FingerprintChangeStart::new(
        ObjectRef::Representation(representation()),
        Some(snapshot.clone()),
        snapshot,
        None,
        0,
        Some(marker),
        false,
    )
    .unwrap()
}

fn insert_current(connection: &Connection, sequence: i64) {
    connection
        .execute(
            "INSERT INTO representation_fingerprint_recomputations VALUES (?1, ?2, ?3)",
            params![
                representation().as_bytes().as_slice(),
                resource().as_bytes().as_slice(),
                sequence
            ],
        )
        .unwrap();
}
