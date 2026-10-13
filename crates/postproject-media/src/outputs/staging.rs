//! Owned staging keeps partial preparation errors from publishing facts.

use super::{PreparedOutput, PreparedOutputs};
use postproject_core::{Error, ErrorKind, Result};

impl PreparedOutputs {
    /// Stages the complete operation, returning the still-uncommitted edit.
    ///
    /// Failure rolls back the owned edit, including any prior staged work.
    /// # Errors
    /// Returns storage, identity, graph or closed-edit errors; publishes nothing.
    pub fn stage<T: postproject_core::ProductionStoreTransaction>(self, edit: T) -> Result<T> {
        self.stage_inner(edit, None)
    }

    /// Stages the one requested output with a real job's guarded completion.
    ///
    /// Job requests have exactly one output. Retain the lease through commit.
    /// Failure rolls back the owned edit; no synthetic job is created.
    /// # Errors
    /// Rejects multiple/new-asset outputs, fencing or input guards, and storage errors.
    pub fn stage_with_job<T: postproject_core::ProductionStoreTransaction>(
        self,
        edit: T,
        lease: &T::Lease,
    ) -> Result<T> {
        self.stage_inner(edit, Some(lease))
    }

    fn stage_inner<T: postproject_core::ProductionStoreTransaction>(
        self,
        mut edit: T,
        lease: Option<&T::Lease>,
    ) -> Result<T> {
        let mut stage = || -> Result<()> {
            if let Some(lease) = lease {
                let [PreparedOutput::Representation(output)] = self.outputs.as_slice() else {
                    return Err(Error::new(
                        ErrorKind::InvalidArgument,
                        "job completion requires one output on its requested asset",
                    ));
                };
                edit.complete_job_lease(lease, output, &self.activity)?;
            } else {
                for output in &self.outputs {
                    match output {
                        PreparedOutput::Original(output) => edit.import_original(output)?,
                        PreparedOutput::Representation(output) => {
                            edit.add_representation(output)?;
                        }
                    }
                }
                edit.create_activity(&self.activity)?;
            }
            for (property, value) in &self.parameters {
                edit.add_metadata_value(
                    postproject_core::ObjectRef::Activity(self.activity.id()),
                    property,
                    value,
                )?;
            }
            Ok(())
        };
        if let Err(error) = stage() {
            edit.rollback()?;
            return Err(error);
        }
        Ok(edit)
    }
}
