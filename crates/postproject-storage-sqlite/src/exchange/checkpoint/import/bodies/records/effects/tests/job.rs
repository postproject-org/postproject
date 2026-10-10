mod integrity;

use std::time::Duration;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ContentStructure,
    JobFailure, Locator, LocatorAvailability, LocatorId, MetadataProperty, MetadataValue,
    ObjectRef, PropertyId, Representation, RepresentationId, RepresentationImport,
    RepresentationKind, Resource, ResourceId, ToolIdentity, VocabularyId,
};

#[test]
fn retained_requests_inert_claims_cancellation_and_removed_target_roots_match_current_jobs() {
    let directory = tempfile::tempdir().unwrap();
    let (source, _, _) = crate::exchange::checkpoint::sections::jobs::tests::fixture(
        &directory.path().join("source.pproj"),
    );
    for floor in [0, 1, 2, 3] {
        super::media::audit(&source, floor, 3);
    }
}

#[test]
fn retained_renewal_release_reclaim_failure_and_completion_keep_original_publication_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let (source, _) = publication_fixture(&directory.path().join("complete.pproj"));
    for floor in [0, 2, 3, 4, 5, 6] {
        super::media::audit(&source, floor, 6);
    }
}

fn publication_fixture(
    path: &std::path::Path,
) -> (crate::SqliteProduction, Vec<postproject_core::Job>) {
    let (mut source, jobs, token) =
        crate::exchange::checkpoint::sections::jobs::tests::fixture(path);
    let lease = source.import_job_lease(&token).unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.renew_job_lease(&lease, Duration::from_micros(200))
        .unwrap();
    edit.release_job_lease(&lease).unwrap();
    edit.commit().unwrap();
    drop(edit);
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    let tool = ToolIdentity::new(" Different 名 ", None, None).unwrap();
    let complete = edit
        .claim_job_lease(jobs[0].id(), &tool, None, Duration::from_micros(100))
        .unwrap();
    let fail = edit
        .claim_job_lease(jobs[2].id(), &tool, None, Duration::from_micros(100))
        .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let resource = ResourceId::new();
    let representation = Representation::new(
        RepresentationId::new(),
        jobs[0].requested_output().asset_id(),
        RepresentationKind::Derived,
        ContentStructure::single_resource(resource),
        Vec::new(),
    );
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("unknown:Render").unwrap(),
        jobs[0]
            .inputs()
            .iter()
            .map(|id| ActivityInput::new(*id, None))
            .collect(),
        vec![ActivityOutput::new(representation.id(), None)],
    )
    .unwrap();
    let output = RepresentationImport::new(
        representation,
        vec![Resource::new(resource, Vec::new(), None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///missing-derived",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let base = source.read_session().unwrap().decision_base();
    let mut edit = source.begin_edit(base).unwrap();
    edit.complete_job_lease(&complete, &output, &activity)
        .unwrap();
    edit.fail_job_lease(&fail, &JobFailure::new(" Exact failure 名 ").unwrap())
        .unwrap();
    edit.add_metadata_value(
        ObjectRef::Job(jobs[0].id()),
        &MetadataProperty::new(
            VocabularyId::new("unknown:CASE").unwrap(),
            PropertyId::new("Exact").unwrap(),
        ),
        &MetadataValue::u64(u64::MAX),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    (source, jobs)
}
