use std::time::Duration;

use postproject_core::{
    Job, JobId, JobKind, JobState, RepresentationId, RepresentationKind, RequestedJobOutput,
    ToolIdentity,
};
use postproject_protocol::{CheckpointSection, Document, JobHeader, JobInput};
use postproject_storage_sqlite::{CheckpointLimits, SqliteProduction};

#[test]
fn rehashed_current_job_contradictions_publish_no_destination() {
    let directory = tempfile::tempdir().unwrap();
    let (mut source, media) = super::media::fixture(&directory.path().join("source.pproj"));
    let job = Job::new(
        JobId::new(),
        JobKind::new("unknown:Exact").unwrap(),
        vec![media.representation().id()],
        RequestedJobOutput::new(media.asset().id(), RepresentationKind::Derived, None).unwrap(),
    )
    .unwrap();
    let mut edit = source.begin_transaction().unwrap();
    edit.request_job(&job).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    let lease = edit
        .claim_job_lease(
            job.id(),
            &ToolIdentity::new(" Exact 名 ", None, None).unwrap(),
            None,
            Duration::from_secs(3600),
        )
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let token = lease.export_token().unwrap();
    let mut chunks = Vec::new();
    let manifest = source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let frames = super::documents(&chunks, CheckpointSection::Jobs);
    assert_eq!(frames.len(), 2);
    let (rewritten, body) =
        super::replace_section(&manifest, &chunks, CheckpointSection::Jobs, &frames);
    let mut control = SqliteProduction::import_checkpoint(
        directory.path().join("control.pproj"),
        &rewritten,
        body.into_iter().map(Ok),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        source.job(job.id()).unwrap(),
        control.job(job.id()).unwrap()
    );
    assert!(control.import_job_lease(&token).is_err());
    let head = source.exchange_head().unwrap();
    for frames in contradictions(&frames) {
        let (rewritten, body) =
            super::replace_section(&manifest, &chunks, CheckpointSection::Jobs, &frames);
        let destination = directory.path().join("rejected.pproj");
        assert!(
            SqliteProduction::import_checkpoint(
                &destination,
                &rewritten,
                body.into_iter().map(Ok),
                CheckpointLimits::default()
            )
            .is_err()
        );
        assert!(!destination.exists());
        assert_eq!(source.exchange_head().unwrap(), head);
        assert_eq!(
            source.job(job.id()).unwrap(),
            control.job(job.id()).unwrap()
        );
    }
    assert!(std::fs::read_dir(directory.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".postproject-import-")
    }));
}

fn contradictions(frames: &[Document]) -> Vec<Vec<Document>> {
    let header = JobHeader::from_document(&frames[0]).unwrap();
    let input = JobInput::from_document(&frames[1]).unwrap();
    let mut variants = vec![
        Vec::new(),
        frames[..1].to_vec(),
        vec![frames[1].clone(), frames[0].clone()],
        [frames, frames].concat(),
    ];
    for (kind, count, state) in [
        (
            header.kind().clone(),
            header.input_count(),
            JobState::Requested,
        ),
        (
            header.kind().clone(),
            header.input_count(),
            JobState::Cancelled,
        ),
        (
            JobKind::new("unknown:Different").unwrap(),
            header.input_count(),
            header.state().clone(),
        ),
        (header.kind().clone(), 0, header.state().clone()),
        (header.kind().clone(), 2, header.state().clone()),
    ] {
        let changed = JobHeader::new(
            header.id(),
            kind,
            count,
            header.requested_output().clone(),
            state,
        )
        .unwrap();
        variants.push(vec![changed.document().unwrap(), frames[1].clone()]);
    }
    for changed in [
        JobInput::new(input.job_id(), 1, input.representation_id()).unwrap(),
        JobInput::new(JobId::new(), 0, input.representation_id()).unwrap(),
        JobInput::new(input.job_id(), 0, RepresentationId::new()).unwrap(),
    ] {
        variants.push(vec![frames[0].clone(), changed.document()]);
    }
    variants
}
