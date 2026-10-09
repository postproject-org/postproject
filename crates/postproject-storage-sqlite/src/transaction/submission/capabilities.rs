//! Private local context is bound before any current ownership is evaluated.

use std::collections::BTreeMap;

use postproject_core::{Error, ErrorKind, JobClaimId, JobId, ProductionId, Result};
use postproject_protocol::MAX_PROPOSAL_COMMANDS;

use crate::{SqliteJobLease, job_lease::parse_token};

pub(super) enum Capability<'a> {
    Borrowed(&'a SqliteJobLease),
    Token {
        production: ProductionId,
        job: JobId,
        secret: JobClaimId,
    },
}

impl Capability<'_> {
    fn identity(&self) -> (ProductionId, JobId, JobClaimId) {
        match self {
            Self::Borrowed(lease) => (lease.production, lease.job, lease.secret),
            Self::Token {
                production,
                job,
                secret,
            } => (*production, *job, *secret),
        }
    }
}

pub(super) struct CapabilityInput<'a> {
    capabilities: BTreeMap<JobId, Capability<'a>>,
    binding: Option<[u8; 32]>,
}

impl<'a> CapabilityInput<'a> {
    pub fn new(leases: &[&'a SqliteJobLease], tokens: &[&str]) -> Result<Self> {
        if leases.len().saturating_add(tokens.len()) > MAX_PROPOSAL_COMMANDS {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "too many local job capabilities",
            ));
        }
        let mut capabilities = BTreeMap::new();
        for capability in leases
            .iter()
            .map(|lease| Capability::Borrowed(lease))
            .chain(
                tokens
                    .iter()
                    .map(|token| {
                        parse_token(token).map(|(production, job, secret)| Capability::Token {
                            production,
                            job,
                            secret,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            )
        {
            let (_, job, _) = capability.identity();
            if capabilities.insert(job, capability).is_some() {
                return Err(Error::new(
                    ErrorKind::InvalidArgument,
                    "duplicate local job capability",
                ));
            }
        }
        let binding = if capabilities.is_empty() {
            None
        } else {
            let mut hasher = blake3::Hasher::new_derive_key(
                "postproject.exchange.v1.private-capability-binding",
            );
            hasher.update(&1_u64.to_be_bytes());
            hasher.update(
                &u64::try_from(capabilities.len())
                    .map_err(|_| Error::new(ErrorKind::Internal, "capability count overflow"))?
                    .to_be_bytes(),
            );
            for capability in capabilities.values() {
                let (production, job, secret) = capability.identity();
                hasher.update(production.as_bytes());
                hasher.update(job.as_bytes());
                hasher.update(secret.as_bytes());
            }
            Some(*hasher.finalize().as_bytes())
        };
        Ok(Self {
            capabilities,
            binding,
        })
    }

    pub const fn binding(&self) -> Option<&[u8; 32]> {
        self.binding.as_ref()
    }

    pub fn validate_scope(&self, production: ProductionId) -> Result<()> {
        if self
            .capabilities
            .values()
            .any(|capability| capability.identity().0 != production)
        {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                "job capability belongs to another production",
            ));
        }
        Ok(())
    }

    pub fn get(&self, job: JobId) -> Result<&Capability<'a>> {
        self.capabilities.get(&job).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidArgument,
                "job command requires local worker ownership",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use postproject_core::{JobLeaseState, Timestamp};

    fn lease() -> SqliteJobLease {
        SqliteJobLease::imported(
            ProductionId::new(),
            JobId::new(),
            JobClaimId::new(),
            Timestamp::from_unix_micros(-1),
        )
    }

    #[test]
    fn binding_is_complete_order_independent_and_identical_for_borrowed_or_encoded_ownership() {
        let first = lease();
        let second = lease();
        let tokens = [
            first.export_token().unwrap(),
            second.export_token().unwrap(),
        ];
        let borrowed = CapabilityInput::new(&[&first, &second], &[]).unwrap();
        let encoded = CapabilityInput::new(&[], &[&tokens[1], &tokens[0]]).unwrap();
        assert_eq!(borrowed.binding(), encoded.binding());
        assert_ne!(
            CapabilityInput::new(&[&first], &[]).unwrap().binding(),
            borrowed.binding()
        );
        *first.state.lock().unwrap() = JobLeaseState::Closed;
        // Binding never reevaluates expiry or cached ownership on a retry.
        assert_eq!(
            CapabilityInput::new(&[&second, &first], &[])
                .unwrap()
                .binding(),
            encoded.binding()
        );
        assert_eq!(CapabilityInput::new(&[], &[]).unwrap().binding(), None);
        assert!(borrowed.validate_scope(second.production).is_err());
    }

    #[test]
    fn malformed_duplicate_and_oversized_private_context_are_bounded_and_redacted() {
        let lease = lease();
        let token = lease.export_token().unwrap();
        for result in [
            CapabilityInput::new(&[&lease, &lease], &[]),
            CapabilityInput::new(&[&lease], &[&token]),
            CapabilityInput::new(&[], &["invalid"]),
            CapabilityInput::new(&vec![&lease; MAX_PROPOSAL_COMMANDS + 1], &[]),
        ] {
            let Err(error) = result else {
                panic!("invalid context accepted")
            };
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(!error.to_string().contains(&lease.secret.to_string()));
            assert!(!error.to_string().contains(&token));
        }
    }
}
