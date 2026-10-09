//! Fresh completion retains original input guards before publishing any facts.

use std::time::Duration;

use postproject_core::{
    ErrorKind, JobState, MetadataProperty, MetadataValue, ObjectRef, PropertyId,
    ResourceFingerprint, ToolIdentity, VocabularyId,
};
use postproject_protocol::{Command, OutcomeStatus, RejectionKind};

use super::Fixture;

#[test]
fn changed_inputs_reject_complete_submission_and_recover_after_cancellation() {
    let mut fixture = Fixture::new();
    let claim = fixture.proposal(vec![Command::ClaimJob {
        job_id: fixture.job.id(),
        tool: ToolIdentity::new("worker", None, None).unwrap(),
        agent: None,
        duration: Duration::from_micros(100),
    }]);
    let (_, mut leases) = fixture
        .source
        .submit_proposal_with_capabilities(&claim, &[], &[])
        .unwrap()
        .into_parts();
    let lease = leases.pop().unwrap();
    let token = lease.export_token().unwrap();
    let resource = fixture.source.resources(fixture.job.inputs()[0]).unwrap()[0].id();
    let base = fixture.source.read_session().unwrap().decision_base();
    let mut edit = fixture
        .source
        .begin_transaction_at(base.revision_id().unwrap())
        .unwrap();
    edit.record_resource_fingerprint(
        resource,
        &ResourceFingerprint::new("unknown", 1, vec![7]).unwrap(),
    )
    .unwrap();
    edit.commit().unwrap();
    drop(edit);
    let property = MetadataProperty::new(
        VocabularyId::new("urn:submission").unwrap(),
        PropertyId::new("pending").unwrap(),
    );
    let target = ObjectRef::Production(fixture.source.production().id());
    let completion = fixture.proposal(vec![
        Command::AppendMetadata {
            target,
            property: property.clone(),
            value: MetadataValue::boolean(true),
        },
        Command::CompleteJob {
            job_id: fixture.job.id(),
            output: Box::new(fixture.output.clone()),
            activity: Box::new(fixture.activity.clone()),
        },
    ]);
    let result = fixture
        .source
        .submit_proposal_with_capabilities(&completion, &[], &[&token])
        .unwrap();
    assert_eq!(result.leases().len(), 0);
    let outcome = result.into_parts().0;
    assert!(
        matches!(outcome.status(), OutcomeStatus::Rejected(rejection) if rejection.kind() == RejectionKind::Domain(ErrorKind::Conflict))
    );
    assert_eq!(outcome.jobs(), []);
    assert_eq!(
        fixture.source.metadata_values(target, &property).unwrap(),
        []
    );
    assert_eq!(
        fixture
            .source
            .representation(fixture.output.representation().id())
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
    assert_eq!(fixture.source.activities().unwrap(), []);
    assert!(matches!(
        fixture.source.job(fixture.job.id()).unwrap().state(),
        JobState::Claimed(_)
    ));
    assert_eq!(fixture.source.exchange_head().unwrap().sequence(), 3);
    let cancel = fixture.proposal(vec![Command::CancelJob(fixture.job.id())]);
    fixture.source.submit_proposal(&cancel).unwrap();
    fixture.reopen();
    fixture.clock.disable();
    let duplicate = fixture
        .source
        .submit_proposal_with_capabilities(&completion, &[], &[&token])
        .unwrap();
    assert_eq!(duplicate.outcome(), &outcome);
    assert_eq!(duplicate.leases().len(), 0);
    assert_eq!(fixture.source.exchange_head().unwrap().sequence(), 4);
}
