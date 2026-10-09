use postproject_core::{AssetId, ExternalIdentifier, IdentifierScheme, ObjectRef};
use postproject_protocol::{IdentifierAttachment, IdentifierChange};
use rusqlite::{Connection, params};

#[test]
fn exact_attachment_removal_and_readdition_preserve_the_new_insertion_order() {
    let connection = fixture();
    let target = ObjectRef::Asset(AssetId::new());
    let first = attachment(target, "First 名", None);
    let second = attachment(target, "Second", Some("Qual"));
    for change in [
        IdentifierChange::Added(first.clone()),
        IdentifierChange::Added(second.clone()),
        IdentifierChange::Removed(first.clone()),
        IdentifierChange::Added(first.clone()),
    ] {
        super::change(&connection, &change, true).unwrap();
    }
    insert(&connection, &second);
    insert(&connection, &first);
    assert!(super::change(&connection, &IdentifierChange::Added(first.clone()), true).is_err());
    super::finish(&connection, true).unwrap();
    assert!(!connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'checkpoint_identifier_state')", [], |row| row.get::<_, bool>(0)).unwrap());
}

#[test]
fn migration_prefixes_are_retained_but_cannot_follow_authored_additions() {
    let connection = fixture();
    let target = ObjectRef::Asset(AssetId::new());
    let baseline = attachment(target, "Baseline", None);
    let added = attachment(target, "Added", None);
    insert(&connection, &added);
    insert(&connection, &baseline);
    super::change(&connection, &IdentifierChange::Added(added.clone()), false).unwrap();
    assert!(super::finish(&connection, false).is_err());
    connection
        .execute("DELETE FROM external_identifiers", [])
        .unwrap();
    insert(&connection, &baseline);
    insert(&connection, &added);
    assert!(super::finish(&connection, true).is_err());
    super::finish(&connection, false).unwrap();
}

#[test]
fn authored_order_and_exact_qualifiers_cannot_be_changed_in_current_state() {
    let connection = fixture();
    let target = ObjectRef::Asset(AssetId::new());
    let plain = attachment(target, "Exact 名", None);
    let qualified = attachment(target, "Exact 名", Some("Qual"));
    super::change(&connection, &IdentifierChange::Added(plain.clone()), true).unwrap();
    super::change(
        &connection,
        &IdentifierChange::Added(qualified.clone()),
        true,
    )
    .unwrap();
    insert(&connection, &qualified);
    insert(&connection, &plain);
    assert!(super::finish(&connection, true).is_err());
    connection
        .execute("DELETE FROM external_identifiers", [])
        .unwrap();
    insert(&connection, &plain);
    insert(&connection, &qualified);
    connection
        .execute(
            "UPDATE external_identifiers SET qualifier = 'qual' WHERE qualifier IS NOT NULL",
            [],
        )
        .unwrap();
    assert!(super::finish(&connection, true).is_err());
    connection
        .execute(
            "UPDATE external_identifiers SET qualifier = 'Qual' WHERE qualifier IS NOT NULL",
            [],
        )
        .unwrap();
    super::finish(&connection, true).unwrap();
}

#[test]
fn removed_baseline_attachments_remain_absent_and_repeated_removal_rejects() {
    let connection = fixture();
    let removed = attachment(
        ObjectRef::Resource(postproject_core::ResourceId::new()),
        "Removed",
        None,
    );
    let change = IdentifierChange::Removed(removed.clone());
    assert!(super::change(&connection, &change, true).is_err());
    super::change(&connection, &change, false).unwrap();
    assert!(super::change(&connection, &change, false).is_err());
    insert(&connection, &removed);
    assert!(super::finish(&connection, false).is_err());
    connection
        .execute("DELETE FROM external_identifiers", [])
        .unwrap();
    super::finish(&connection, false).unwrap();
}

fn fixture() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection.execute_batch("CREATE TABLE external_identifiers (id INTEGER PRIMARY KEY, target_kind INTEGER NOT NULL, target_id BLOB NOT NULL, scheme TEXT NOT NULL, value TEXT NOT NULL, qualifier TEXT);").unwrap();
    super::create(&connection).unwrap();
    connection
}

fn attachment(target: ObjectRef, value: &str, qualifier: Option<&str>) -> IdentifierAttachment {
    IdentifierAttachment::new(
        target,
        ExternalIdentifier::new(
            IdentifierScheme::new("unknown:CASE").unwrap(),
            value,
            qualifier.map(str::to_owned),
        )
        .unwrap(),
    )
    .unwrap()
}

fn insert(connection: &Connection, attachment: &IdentifierAttachment) {
    let target = attachment.target();
    let (kind, id) = crate::encode_metadata_target(&target).unwrap();
    let identifier = attachment.identifier();
    connection.execute("INSERT INTO external_identifiers (target_kind, target_id, scheme, value, qualifier) VALUES (?1, ?2, ?3, ?4, ?5)", params![kind, id.as_slice(), identifier.scheme().as_str(), identifier.value(), identifier.qualifier()]).unwrap();
}
