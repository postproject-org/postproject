use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, AgentIdentity, Asset,
    AssetId, ContentStructure, ExternalIdentifier, IdentifierScheme, Locator, LocatorAvailability,
    LocatorId, OriginalMediaImport, Representation, RepresentationFingerprint, RepresentationId,
    RepresentationKind, Resource, ResourceId, Timestamp, ToolIdentity,
};
use postproject_protocol::{
    ActivityHeader, CheckpointId, CheckpointSection, FrameDecoder, Limits,
    decode_fingerprint_snapshot,
};

use crate::{
    SqliteProduction,
    exchange::checkpoint::{sections, writer::SectionWriter},
};

mod staging;

#[test]
fn streamed_activity_keeps_authored_attribution_and_original_fingerprints() {
    let directory = tempfile::tempdir().unwrap();
    let (source, activity, _) = source(directory.path());
    assert_stream(&source, &activity);
    source
        .connection
        .execute(
            "DELETE FROM activity_outputs WHERE activity_id = ?1",
            [activity.id().as_bytes().as_slice()],
        )
        .unwrap();
    assert!(super::header(&source.connection, activity.id()).is_err());
}

fn source(directory: &std::path::Path) -> (SqliteProduction, Activity, [OriginalMediaImport; 2]) {
    let mut source = SqliteProduction::create(directory.join("source.pproj"), None).unwrap();
    let first = media();
    let second = media();
    let activity = activity(first.representation().id(), second.representation().id());
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.import_original(&first).unwrap();
    edit.import_original(&second).unwrap();
    edit.create_activity(&activity).unwrap();
    edit.record_representation_fingerprint(
        first.representation().id(),
        &RepresentationFingerprint::new("unknown_CASE", 1, vec![99]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    (source, activity, [first, second])
}

fn activity(input: RepresentationId, output: RepresentationId) -> Activity {
    Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .unwrap()
    .with_timing(
        Some(Timestamp::from_unix_micros(i64::MIN)),
        Some(Timestamp::from_unix_micros(i64::MAX)),
    )
    .unwrap()
    .with_tool(
        ToolIdentity::new(
            "  Exact 名  ",
            Some(" exact ".into()),
            Some("unknown:CASE".into()),
        )
        .unwrap(),
    )
    .with_agent(
        AgentIdentity::new(
            Some(" artist 名 ".into()),
            Some(
                ExternalIdentifier::new(
                    IdentifierScheme::new("unknown:CASE").unwrap(),
                    "  Exact  ",
                    Some(" A ".into()),
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn assert_stream(source: &SqliteProduction, activity: &Activity) {
    let expected = ActivityHeader::from_activity(activity).unwrap();
    assert_eq!(
        super::header(&source.connection, activity.id()).unwrap(),
        expected
    );
    let mut chunks = Vec::new();
    let mut sink = |chunk| {
        chunks.push(chunk);
        Ok(())
    };
    let mut writer = SectionWriter::new(
        source.exchange_scope().unwrap(),
        CheckpointId::new(),
        CheckpointSection::Activities,
        &mut sink,
    );
    sections::activities(source, &mut writer).unwrap();
    assert_eq!(writer.finish().unwrap().items(), 1);
    let mut decoder = FrameDecoder::new(Limits::default());
    let mut frames = Vec::new();
    for chunk in chunks {
        let mut offset = 0;
        while offset < chunk.payload().len() {
            let (consumed, frame) = decoder.consume(&chunk.payload()[offset..]).unwrap();
            offset += consumed;
            frames.extend(frame);
        }
    }
    decoder.finish().unwrap();
    assert_eq!(frames[0], expected.document());
    let snapshots: Vec<_> = frames
        .iter()
        .filter(|frame| frame.kind().unwrap() == "fingerprint.snapshot")
        .map(|frame| decode_fingerprint_snapshot(frame).unwrap())
        .collect();
    assert_eq!(snapshots.len(), 2);
    assert!(snapshots.iter().all(
        |snapshot| snapshot.value() == [1] && snapshot.observed_revision_sequence() == Some(1)
    ));
}

fn media() -> OriginalMediaImport {
    let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
    let resource = ResourceId::new();
    OriginalMediaImport::new(
        asset.clone(),
        Representation::new(
            RepresentationId::new(),
            asset.id(),
            RepresentationKind::Original,
            ContentStructure::single_resource(resource),
            vec![RepresentationFingerprint::new("unknown_CASE", 1, vec![1]).unwrap()],
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///does-not-exist",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
