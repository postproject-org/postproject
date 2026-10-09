use crate::{
    SqliteProduction,
    exchange::checkpoint::{sections, writer::SectionWriter},
};
use postproject_core::{
    Asset, AssetId, ContentStructure, Locator, LocatorAvailability, LocatorId, OriginalMediaImport,
    Representation, RepresentationFingerprint, RepresentationId, RepresentationKind, Resource,
    ResourceFingerprint, ResourceId, Timestamp,
};
use postproject_protocol::{CheckpointId, CheckpointSection, FrameDecoder, Limits};

pub(super) fn populate(source: &mut SqliteProduction, import: &OriginalMediaImport) {
    let mut edit = source.begin_transaction().unwrap();
    edit.import_original(import).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    for byte in [2, 3] {
        edit.record_resource_fingerprint(
            import.resources()[0].id(),
            &ResourceFingerprint::new("unknown_CASE", 65_535, vec![byte]).unwrap(),
        )
        .unwrap();
        edit.record_representation_fingerprint(
            import.representation().id(),
            &RepresentationFingerprint::new("unknown_CASE", 0, vec![byte + 10]).unwrap(),
        )
        .unwrap();
    }
    // A final resource update leaves its exact aggregate recomputation marker.
    edit.record_resource_fingerprint(
        import.resources()[0].id(),
        &ResourceFingerprint::new("unknown_CASE", 65_535, vec![4]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
}

pub(super) fn fixture() -> OriginalMediaImport {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(-1), None, None);
    let resource = Resource::new(
        ResourceId::new(),
        vec![ResourceFingerprint::new("unknown_CASE", 65_535, vec![1]).unwrap()],
        None,
    );
    let representation = Representation::new(
        RepresentationId::new(),
        asset.id(),
        RepresentationKind::Original,
        ContentStructure::single_resource(resource.id()),
        vec![RepresentationFingerprint::new("unknown_CASE", 0, vec![11]).unwrap()],
    );
    let locator = Locator::new(
        LocatorId::new(),
        resource.id(),
        "file:///does-not-exist",
        None,
        LocatorAvailability::Offline,
    )
    .unwrap();
    OriginalMediaImport::new(asset, representation, vec![resource], vec![locator]).unwrap()
}

pub(super) fn exported_documents(source: &SqliteProduction) -> Vec<postproject_protocol::Document> {
    let mut chunks = Vec::new();
    let mut sink = |chunk| {
        chunks.push(chunk);
        Ok(())
    };
    let mut writer = SectionWriter::new(
        source.exchange_head().unwrap().scope(),
        CheckpointId::new(),
        CheckpointSection::Fingerprints,
        &mut sink,
    );
    sections::fingerprints(source, &mut writer).unwrap();
    let summary = writer.finish().unwrap();
    let mut documents = Vec::new();
    let mut decoder = FrameDecoder::new(Limits::default());
    for chunk in chunks {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, document) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            if let Some(document) = document {
                documents.push(document);
            }
        }
    }
    decoder.finish().unwrap();
    assert_eq!(summary.items(), documents.len() as u64);
    documents
}

pub(super) fn portable_rows(
    connection: &rusqlite::Connection,
    table: &str,
) -> Vec<Vec<rusqlite::types::Value>> {
    let (columns, order) = if table.ends_with("recomputations") {
        (
            "representation_id, changed_resource_id, marked_revision_sequence",
            "representation_id",
        )
    } else {
        let owner = if table.starts_with("resource_") {
            "resource_id"
        } else {
            "representation_id"
        };
        let columns = format!(
            "{owner}, algorithm, algorithm_version, value, observed_revision_sequence{}",
            if table.ends_with("history") {
                ", superseded_revision_sequence"
            } else {
                ""
            }
        );
        return queried_rows(
            connection,
            table,
            &columns,
            &format!(
                "{owner}, algorithm, algorithm_version{}",
                if table.ends_with("history") {
                    ", id"
                } else {
                    ""
                }
            ),
        );
    };
    queried_rows(connection, table, columns, order)
}

pub(super) fn queried_rows(
    connection: &rusqlite::Connection,
    table: &str,
    columns: &str,
    order: &str,
) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = connection
        .prepare(&format!("SELECT {columns} FROM {table} ORDER BY {order}"))
        .unwrap();
    let count = statement.column_count();
    statement
        .query_map([], |row| (0..count).map(|column| row.get(column)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
