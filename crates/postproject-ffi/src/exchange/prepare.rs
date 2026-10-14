//! Translate already checked native staging without applying or recapturing it.

use postproject_protocol::{Command, IdentifierAttachment, Result};

use crate::{StagedMutation, job_leases::LeaseMutation};

pub(super) fn command(mutation: &StagedMutation) -> Result<Command> {
    Ok(match mutation {
        StagedMutation::Import(value) => Command::ImportOriginal(value.clone()),
        StagedMutation::Representation(value) => Command::AddRepresentation(value.clone()),
        StagedMutation::MediaRoot(value) => Command::AddMediaRoot(value.clone()),
        StagedMutation::SetMediaRootEnabled(root_id, enabled) => Command::SetMediaRootEnabled {
            root_id: *root_id,
            enabled: *enabled,
        },
        StagedMutation::RemoveMediaRoot(id) => Command::RemoveMediaRoot(*id),
        StagedMutation::Locator(value) => Command::AddLocator(value.clone()),
        StagedMutation::ConfirmedLocator(value) => Command::ConfirmLocator(value.clone()),
        StagedMutation::RetireLocator(id) => Command::RetireLocator(*id),
        StagedMutation::RecordResourceFingerprint(resource_id, fingerprint) => {
            Command::RecordResourceFingerprint {
                resource_id: *resource_id,
                fingerprint: fingerprint.clone(),
            }
        }
        StagedMutation::RecordRepresentationFingerprint(representation_id, fingerprint) => {
            Command::RecordRepresentationFingerprint {
                representation_id: *representation_id,
                fingerprint: fingerprint.clone(),
            }
        }
        StagedMutation::RecordResourceFileFacts(resource_id, facts) => {
            Command::RecordResourceFileFacts {
                resource_id: *resource_id,
                facts: *facts,
            }
        }
        StagedMutation::RecordDependencySet(representation_id, dependencies) => {
            Command::RecordDependencySet {
                representation_id: *representation_id,
                dependencies: dependencies.clone(),
            }
        }
        StagedMutation::AddExternalIdentifier(target, identifier) => {
            Command::AddIdentifier(IdentifierAttachment::new(*target, identifier.clone())?)
        }
        StagedMutation::RemoveExternalIdentifier(target, identifier) => {
            Command::RemoveIdentifier(IdentifierAttachment::new(*target, identifier.clone())?)
        }
        StagedMutation::AddMetadataValue(target, property, value) => Command::AppendMetadata {
            target: *target,
            property: property.clone(),
            value: value.clone(),
        },
        StagedMutation::RemoveMetadataProperty(target, property) => Command::RemoveMetadata {
            target: *target,
            property: property.clone(),
        },
        StagedMutation::Activity(value) => Command::CreateActivity(value.clone()),
        StagedMutation::RequestJob(value) => Command::RequestJob(value.clone()),
        StagedMutation::CancelJob(id) => Command::CancelJob(*id),
        StagedMutation::Lease(value) => lease_command(value),
    })
}

fn lease_command(mutation: &LeaseMutation) -> Command {
    match mutation {
        LeaseMutation::Claim {
            lease,
            tool,
            agent,
            duration,
        } => Command::ClaimJob {
            job_id: lease.job,
            tool: tool.clone(),
            agent: agent.clone(),
            duration: *duration,
        },
        LeaseMutation::Renew(lease, duration) => Command::RenewJob {
            job_id: lease.job,
            duration: *duration,
        },
        LeaseMutation::Release(lease) => Command::ReleaseJob(lease.job),
        LeaseMutation::Fail(lease, failure) => Command::FailJob {
            job_id: lease.job,
            failure: failure.clone(),
        },
        LeaseMutation::Complete {
            lease,
            output,
            activity,
        } => Command::CompleteJob {
            job_id: lease.job,
            output: Box::new(output.clone()),
            activity: activity.clone(),
        },
    }
}
