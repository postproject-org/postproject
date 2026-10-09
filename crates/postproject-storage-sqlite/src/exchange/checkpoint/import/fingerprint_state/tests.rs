use postproject_core::{FingerprintSnapshot, ObjectRef, RepresentationId, ResourceId};
use postproject_protocol::{FingerprintChangeStart, FingerprintObservation, FingerprintState};
use rusqlite::{Connection, params};

#[test]
fn initial_and_intermediate_observations_explain_every_exact_archive_and_current_value() {
    let connection = fixture();
    history(&connection, 1, Some(1), 2);
    history(&connection, 2, Some(2), 2);
    current(&connection, 3, Some(2));
    super::create(&connection).unwrap();
    let initial =
        FingerprintObservation::new(target(), snapshot(1, Some(1)), FingerprintState::Current)
            .unwrap();
    super::initial(&connection, &initial).unwrap();
    assert!(super::initial(&connection, &initial).is_err());
    assert!(super::finish(&connection, 0).is_err());
    super::changed(&connection, &change(1, Some(1), 2, Some(0)), 2, 0).unwrap();
    assert!(super::finish(&connection, 0).is_err());
    super::changed(&connection, &change(2, Some(2), 3, Some(1)), 2, 0).unwrap();
    super::finish(&connection, 0).unwrap();
}

#[test]
fn migration_prefixes_keep_unknown_observation_boundaries_and_original_archive_positions() {
    let connection = fixture();
    history(&connection, 9, None, 1);
    history(&connection, 1, Some(1), 2);
    current(&connection, 2, Some(2));
    super::create(&connection).unwrap();
    assert!(super::changed(&connection, &change(1, Some(1), 2, Some(0)), 2, 1).is_err());
    super::changed(&connection, &change(1, Some(1), 2, Some(1)), 2, 1).unwrap();
    super::finish(&connection, 1).unwrap();
}

#[test]
fn altered_previous_bytes_missing_changes_and_wrong_revision_boundaries_reject() {
    let connection = fixture();
    history(&connection, 1, Some(1), 2);
    current(&connection, 2, Some(2));
    super::create(&connection).unwrap();
    assert!(super::changed(&connection, &change(9, Some(1), 2, Some(0)), 2, 1).is_err());
    assert!(super::changed(&connection, &change(1, Some(1), 2, Some(0)), 3, 1).is_err());
    assert!(super::finish(&connection, 1).is_err());
    super::changed(&connection, &change(1, Some(1), 2, Some(0)), 2, 1).unwrap();
    super::finish(&connection, 1).unwrap();
}

#[test]
fn new_domains_need_their_authored_first_observation_and_baseline_domains_need_no_fabrication() {
    let connection = fixture();
    current(&connection, 3, Some(2));
    super::create(&connection).unwrap();
    assert!(super::finish(&connection, 1).is_err());
    let first =
        FingerprintChangeStart::new(target(), None, snapshot(3, Some(2)), None, 0, None, false)
            .unwrap();
    super::changed(&connection, &first, 2, 1).unwrap();
    super::finish(&connection, 1).unwrap();
    connection
        .execute(
            "UPDATE resource_fingerprints SET observed_revision_sequence = NULL",
            [],
        )
        .unwrap();
    super::create(&connection).unwrap();
    assert!(super::finish(&connection, 0).is_err());
    super::finish(&connection, 1).unwrap();
}

#[test]
fn genesis_changes_cannot_invent_an_unknown_precreation_fingerprint() {
    let connection = fixture();
    history(&connection, 1, None, 2);
    current(&connection, 2, Some(2));
    super::create(&connection).unwrap();
    let start = change(1, None, 2, Some(0));
    assert!(super::changed(&connection, &start, 2, 0).is_err());
    super::changed(&connection, &start, 2, 1).unwrap();
    super::finish(&connection, 1).unwrap();
}

#[test]
fn identical_uuid_and_algorithm_do_not_merge_resource_and_representation_evidence() {
    let connection = fixture();
    current(&connection, 1, Some(1));
    connection.execute("INSERT INTO representation_fingerprints (representation_id, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, 'Exact_CASE', 65535, ?2, 1)", params![[1_u8; 16].as_slice(), vec![2_u8]]).unwrap();
    super::create(&connection).unwrap();
    let wrong =
        FingerprintObservation::new(target(), snapshot(2, Some(1)), FingerprintState::Current)
            .unwrap();
    assert!(super::initial(&connection, &wrong).is_err());
    let resource =
        FingerprintObservation::new(target(), snapshot(1, Some(1)), FingerprintState::Current)
            .unwrap();
    super::initial(&connection, &resource).unwrap();
    assert!(super::finish(&connection, 0).is_err());
    let representation = FingerprintObservation::new(
        ObjectRef::Representation(RepresentationId::from_bytes([1; 16])),
        snapshot(2, Some(1)),
        FingerprintState::Current,
    )
    .unwrap();
    super::initial(&connection, &representation).unwrap();
    super::finish(&connection, 0).unwrap();
}

fn fixture() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    for (kind, owner) in [
        ("resource", "resource_id"),
        ("representation", "representation_id"),
    ] {
        connection.execute_batch(&format!("CREATE TABLE {kind}_fingerprints ({owner} BLOB, algorithm TEXT, algorithm_version INTEGER, value BLOB, observed_revision_sequence INTEGER); CREATE TABLE {kind}_fingerprint_history (id INTEGER PRIMARY KEY, {owner} BLOB, algorithm TEXT, algorithm_version INTEGER, value BLOB, observed_revision_sequence INTEGER, superseded_revision_sequence INTEGER);")).unwrap();
    }
    connection
}

fn target() -> ObjectRef {
    ObjectRef::Resource(ResourceId::from_bytes([1; 16]))
}

fn snapshot(value: u8, observed: Option<u64>) -> FingerprintSnapshot {
    FingerprintSnapshot::new("Exact_CASE", 65_535, vec![value], observed).unwrap()
}

fn change(
    previous: u8,
    observed: Option<u64>,
    value: u8,
    position: Option<u64>,
) -> FingerprintChangeStart {
    FingerprintChangeStart::new(
        target(),
        Some(snapshot(previous, observed)),
        snapshot(value, Some(2)),
        position,
        0,
        None,
        false,
    )
    .unwrap()
}

fn history(connection: &Connection, value: u8, observed: Option<i64>, superseded: i64) {
    connection.execute("INSERT INTO resource_fingerprint_history (resource_id, algorithm, algorithm_version, value, observed_revision_sequence, superseded_revision_sequence) VALUES (?1, 'Exact_CASE', 65535, ?2, ?3, ?4)", params![[1_u8; 16].as_slice(), vec![value], observed, superseded]).unwrap();
}

fn current(connection: &Connection, value: u8, observed: Option<i64>) {
    connection.execute("INSERT INTO resource_fingerprints (resource_id, algorithm, algorithm_version, value, observed_revision_sequence) VALUES (?1, 'Exact_CASE', 65535, ?2, ?3)", params![[1_u8; 16].as_slice(), vec![value], observed]).unwrap();
}
