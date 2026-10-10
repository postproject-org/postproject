mod integrity;

use std::{sync::Arc, time::Duration};

use postproject_core::{
    Asset, AssetId, ContentStructure, Error, ErrorKind, Job, JobId, JobKind, JobState, Locator,
    LocatorAvailability, LocatorId, MediaRoot, MediaRootId, OriginalMediaImport, Representation,
    RepresentationId, RepresentationKind, RequestedJobOutput, Resource, ResourceId, Timestamp,
    ToolIdentity,
};
use postproject_protocol::{
    CheckpointId, CheckpointSection, Document, FrameDecoder, JobHeader, JobInput, Limits,
};

use crate::{
    ReplayLimits, SqliteProduction,
    exchange::{checkpoint::writer::SectionWriter, records::JobApply},
    job_clock::JobClock,
};

#[derive(Debug)]
struct FixedClock;
impl JobClock for FixedClock {
    fn now(&self) -> postproject_core::Result<Timestamp> {
        Ok(Timestamp::from_unix_micros(100))
    }
}

#[derive(Debug)]
struct UnavailableClock;
impl JobClock for UnavailableClock {
    fn now(&self) -> postproject_core::Result<Timestamp> {
        Err(Error::new(
            ErrorKind::Internal,
            "mirror has no clock authority",
        ))
    }
}

#[test]
fn current_job_stream_is_bounded_and_claims_are_inert_without_clock_or_secret() {
    let directory = tempfile::tempdir().unwrap();
    let (source, jobs, token) = fixture(&directory.path().join("source.pproj"));
    let frames = frames(&source);
    assert_eq!(frames.len(), 3003);
    assert!(frames.iter().all(|frame| {
        !String::from_utf8(frame.canonical_bytes().unwrap())
            .unwrap()
            .contains(&token)
    }));
    let mut destination = SqliteProduction::create_genesis_mirror(
        directory.path().join("stage.pproj"),
        source.production(),
        source.exchange_floor().unwrap(),
    )
    .unwrap();
    destination.job_clock = Arc::new(UnavailableClock);
    let mut reader = source.record_reader(1).unwrap();
    let manifest = reader.manifest().clone();
    destination
        .apply_record(
            &manifest,
            std::iter::from_fn(|| reader.next_chunk().transpose()),
            ReplayLimits::default(),
        )
        .unwrap();
    let transaction = destination.connection.unchecked_transaction().unwrap();
    transaction.execute("DELETE FROM media_roots", []).unwrap();
    let mut pending = None;
    for frame in &frames {
        if let Some(job) = pending.as_mut() {
            if JobApply::input(job, &transaction, JobInput::from_document(frame).unwrap()).unwrap()
            {
                pending = None;
            }
        } else {
            pending =
                JobApply::begin_checkpoint(&transaction, JobHeader::from_document(frame).unwrap())
                    .unwrap();
        }
    }
    assert!(pending.is_none());
    for job in &jobs {
        assert_eq!(
            source.job(job.id()).unwrap(),
            destination.job(job.id()).unwrap()
        );
    }
    assert!(
        matches!(destination.job(jobs[0].id()).unwrap().state(), JobState::Claimed(claim) if claim.expires_at() == Timestamp::from_unix_micros(200))
    );
    let private: (Option<Vec<u8>>, bool) = transaction
        .query_row(
            "SELECT claim_id, claim_inert FROM jobs WHERE id = ?1",
            [jobs[0].id().as_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(private, (None, true));
    transaction.rollback().unwrap();
    assert!(destination.import_job_lease(&token).is_err());
    assert!(
        source.export_checkpoint(|_| Ok(())).is_err(),
        "job profile remains gated"
    );
}

pub(in crate::exchange::checkpoint) fn fixture(
    path: &std::path::Path,
) -> (SqliteProduction, Vec<Job>, String) {
    let mut source = SqliteProduction::create(path, None).unwrap();
    source.job_clock = Arc::new(FixedClock);
    let imports = [media(), media()];
    let root = MediaRoot::new(MediaRootId::new(), "Exact:名", None, None, 0, true).unwrap();
    let mut edit = source.begin_transaction().unwrap();
    for import in &imports {
        edit.import_original(import).unwrap();
    }
    edit.add_media_root(root.clone()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let jobs: Vec<_> = (0..1001)
        .map(|_| {
            Job::new(
                JobId::new(),
                JobKind::new("unknown:Exact").unwrap(),
                imports
                    .iter()
                    .map(|import| import.representation().id())
                    .collect(),
                RequestedJobOutput::new(
                    imports[0].asset().id(),
                    RepresentationKind::Derived,
                    Some(root.name().into()),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    let mut edit = source.begin_transaction().unwrap();
    for job in &jobs {
        edit.request_job(job).unwrap();
    }
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    let lease = edit
        .claim_job_lease(
            jobs[0].id(),
            &ToolIdentity::new(" Exact 名 ", Some(" custom ".into()), None).unwrap(),
            None,
            Duration::from_micros(100),
        )
        .unwrap();
    edit.cancel_job(jobs[1].id()).unwrap();
    edit.remove_media_root(root.id()).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let token = lease.export_token().unwrap();
    (source, jobs, token)
}

fn frames(source: &SqliteProduction) -> Vec<Document> {
    let mut chunks = Vec::new();
    let mut sink = |chunk| {
        chunks.push(chunk);
        Ok(())
    };
    let mut writer = SectionWriter::new(
        source.exchange_scope().unwrap(),
        CheckpointId::new(),
        CheckpointSection::Jobs,
        &mut sink,
    );
    super::jobs(source, &mut writer).unwrap();
    assert_eq!(writer.finish().unwrap().items(), 1001);
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
    frames
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
            Vec::new(),
        ),
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///missing",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
