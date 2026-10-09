use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, RevisionEvent,
    RevisionEventKind,
};
use postproject_protocol::{
    ActivityEdgeHeader, ActivityEdgeSide, Document, Limits, decode_event, encode_event,
};
use postproject_storage_sqlite::{ReplayLimits, SqliteProduction};

use super::support;

#[test]
fn complete_rehashed_effects_and_observations_cannot_introduce_a_provenance_cycle() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let imports = [
        support::media(1, false),
        support::media(2, false),
        support::media(3, false),
    ];
    let mut edit = source.begin_transaction().unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    let mut second_id = None;
    for pair in imports.windows(2) {
        let id = ActivityId::new();
        let activity = Activity::new(
            id,
            ActivityKind::new("unknown:Render").unwrap(),
            vec![ActivityInput::new(pair[0].representation().id(), None)],
            vec![ActivityOutput::new(pair[1].representation().id(), None)],
        )
        .unwrap();
        edit.create_activity(&activity).unwrap();
        second_id = Some(id);
    }
    edit.commit().unwrap();
    drop(edit);
    let id = second_id.unwrap();
    let target = imports[0].representation().id();
    let mut documents = support::frames(&source, 1);
    for document in &mut documents {
        if document.kind().unwrap() == "activity.edge" {
            let header = ActivityEdgeHeader::from_document(document).unwrap();
            if header.activity_id() == id && header.side() == ActivityEdgeSide::Output {
                let mut wire: serde_json::Value =
                    serde_json::from_slice(&document.canonical_bytes().unwrap()).unwrap();
                wire["representation_id"] = target.to_string().into();
                *document = Document::parse(&serde_json::to_vec(&wire).unwrap(), Limits::default())
                    .unwrap();
            }
        } else if document.kind().unwrap() == "observation" {
            let event = decode_event(document).unwrap();
            if matches!(event.kind(), RevisionEventKind::ActivityOutputAdded { activity_id, .. } if *activity_id == id)
            {
                *document = encode_event(&RevisionEvent::new(
                    event.revision_id(),
                    event.position(),
                    RevisionEventKind::ActivityOutputAdded {
                        activity_id: id,
                        representation_id: target,
                        role: None,
                    },
                ))
                .unwrap();
            }
        }
    }
    let original = source.record_reader(1).unwrap().manifest().clone();
    let (manifest, chunk) = support::rehashed(&original, &documents);
    let mut mirror = SqliteProduction::create_genesis_mirror(
        directory.path().join("mirror.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    assert!(
        mirror
            .apply_record(&manifest, [Ok(chunk)], ReplayLimits::default())
            .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap().sequence(), 0);
    assert_eq!(mirror.activities().unwrap(), []);
}
