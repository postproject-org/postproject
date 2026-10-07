//! Completion must not attribute changed input knowledge to earlier work.

use postproject_core::{
    RepresentationId, ResourceId, RevisionId, SemanticConflictKey, TransactionConflict,
};

use super::{
    Error, ErrorKind, JobId, OptionalExtension, Result, SqliteTransaction, params, sqlite_error,
};

// UNION deduplicates cycles. The extra row detects an exceeded read budget.
const INPUT_GRAPH: &str = "WITH RECURSIVE input_graph(representation_id) AS (
    SELECT representation_id FROM job_inputs WHERE job_id = ?1
    UNION
    SELECT CASE d.target_kind WHEN 2 THEN d.target_id ELSE d.resolved_representation_id END
    FROM input_graph i JOIN dependencies d ON d.source_representation_id = i.representation_id
    WHERE d.required = 1 AND (d.target_kind = 2 OR d.resolved_representation_id IS NOT NULL)
    LIMIT 100001)";

impl SqliteTransaction<'_> {
    pub(super) fn guard_claim_inputs(&mut self, job: JobId) -> Result<()> {
        if let Some(base) = self.base_revision {
            self.guard_job_inputs(job, base)?;
        }
        Ok(())
    }

    pub(super) fn guard_completion_inputs(&mut self, job: JobId) -> Result<()> {
        // A new claim in this edit has already checked its read-derived inputs;
        // no writer can intervene before its atomic publication.
        if self.pending_events.iter().any(|event| {
            matches!(event,
            postproject_core::RevisionEventKind::JobClaimed { job_id } if *job_id == job)
        }) {
            return self.guard_claim_inputs(job);
        }
        let (revision, sequence): (Vec<u8>, i64) = self
            .open_transaction()?
            .query_row(
                "SELECT r.id, r.sequence FROM revisions r
             JOIN revision_events e ON e.revision_id = r.id
             WHERE e.kind = 21 AND e.primary_id = ?1
             ORDER BY r.sequence DESC LIMIT 1",
                [job.as_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(sqlite_error("load worker claim decision revision"))?;
        let claim_base = (
            Some(RevisionId::from_bytes(crate::id_bytes(
                revision,
                "claim revision",
            )?)),
            crate::stored_u64(sequence, "claim revision sequence")?,
        );
        let base = self
            .base_revision
            .filter(|(_, sequence)| *sequence < claim_base.1)
            .unwrap_or(claim_base);
        self.guard_job_inputs(job, base)
    }

    fn guard_job_inputs(&mut self, job: JobId, base: (Option<RevisionId>, u64)) -> Result<()> {
        let count: i64 = self
            .open_transaction()?
            .query_row(
                &format!("{INPUT_GRAPH} SELECT COUNT(*) FROM input_graph"),
                [job.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(sqlite_error("bound worker input dependency graph"))?;
        if count > 100_000 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "worker input graph exceeds 100000 representations",
            ));
        }
        // Fetch at most one changed fact, rather than materializing all input
        // resources/fingerprint domains. Key tags match the private encoder.
        let changed: Option<(Vec<u8>, Vec<u8>, i64)> = self.open_transaction()?.query_row(
            &format!("{INPUT_GRAPH} SELECT c.conflict_key, c.last_changed_revision_id, c.last_changed_revision_sequence
             FROM conflict_versions c
             WHERE c.last_changed_revision_sequence > ?2 AND (
                 (substr(c.conflict_key, 1, 1) IN (x'03', x'07') AND EXISTS (
                     SELECT 1 FROM input_graph i WHERE
                     i.representation_id = substr(c.conflict_key, 2, 16))) OR
                 (substr(c.conflict_key, 1, 1) = x'06' AND EXISTS (
                     SELECT 1 FROM input_graph i JOIN representation_resources rr
                         ON rr.representation_id = i.representation_id
                     WHERE rr.resource_id = substr(c.conflict_key, 2, 16))))
             ORDER BY c.last_changed_revision_sequence, c.conflict_key LIMIT 1"),
            params![job.as_bytes().as_slice(), i64::try_from(base.1).map_err(|_| invalid_key())?],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional().map_err(sqlite_error("check worker input decision"))?;
        if let Some((encoded, revision, sequence)) = changed {
            let conflict = TransactionConflict::new(
                decode_key(&encoded)?,
                base.0,
                base.1,
                RevisionId::from_bytes(crate::id_bytes(revision, "changed input revision")?),
                crate::stored_u64(sequence, "changed input revision sequence")?,
            );
            return Err(Error::transaction_conflict(
                conflict,
                "job input knowledge changed after the worker decision",
            ));
        }
        Ok(())
    }
}

fn invalid_key() -> Error {
    Error::new(ErrorKind::Storage, "invalid worker input conflict key")
}

fn decode_key(encoded: &[u8]) -> Result<SemanticConflictKey> {
    if encoded.len() < 17 || encoded.len() > 1_024 {
        return Err(invalid_key());
    }
    let id: [u8; 16] = encoded[1..17].try_into().map_err(|_| invalid_key())?;
    if id == [0; 16] {
        return Err(invalid_key());
    }
    if encoded[0] == 3 && encoded.len() == 17 {
        return Ok(SemanticConflictKey::DependencySet(
            RepresentationId::from_bytes(id),
        ));
    }
    if !matches!(encoded[0], 6 | 7) || encoded.len() < 23 {
        return Err(invalid_key());
    }
    let length = u32::from_be_bytes(encoded[17..21].try_into().map_err(|_| invalid_key())?);
    let length = usize::try_from(length).map_err(|_| invalid_key())?;
    if length != encoded.len() - 23 {
        return Err(invalid_key());
    }
    let algorithm = std::str::from_utf8(&encoded[21..21 + length])
        .map_err(|_| invalid_key())?
        .to_owned();
    let version = u16::from_be_bytes(
        encoded[21 + length..]
            .try_into()
            .map_err(|_| invalid_key())?,
    );
    postproject_core::ResourceFingerprint::new(algorithm.clone(), version, vec![1])
        .map_err(|_| invalid_key())?;
    Ok(if encoded[0] == 6 {
        SemanticConflictKey::ResourceFingerprint {
            resource_id: ResourceId::from_bytes(id),
            algorithm,
            version,
        }
    } else {
        SemanticConflictKey::RepresentationFingerprint {
            representation_id: RepresentationId::from_bytes(id),
            algorithm,
            version,
        }
    })
}
