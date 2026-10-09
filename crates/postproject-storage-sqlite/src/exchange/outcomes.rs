use postproject_core::{Error, ErrorKind};
use postproject_protocol::{
    ClientId, Digest, Document, FailureKind, Outcome, ProtocolError, RequestId, Scope,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::ExchangeResult;
use crate::{SqliteProduction, sqlite_error};

#[cfg(test)]
mod tests;

pub(crate) fn validate_scope(connection: &Connection, expected: Scope) -> ExchangeResult<()> {
    if super::scope(connection, crate::load_production(connection)?.id())? != expected {
        return Err(ProtocolError::new(
            FailureKind::ScopeMismatch,
            "request belongs to another production or history",
        )
        .into());
    }
    Ok(())
}

pub(crate) fn lookup(
    connection: &Connection,
    scope: Scope,
    client: ClientId,
    request: RequestId,
) -> ExchangeResult<Option<Outcome>> {
    read(connection, scope, client, request).map(|stored| stored.map(|(outcome, _)| outcome))
}

pub(crate) fn lookup_for_submission(
    connection: &Connection,
    scope: Scope,
    client: ClientId,
    request: RequestId,
    digest: Digest,
    binding: Option<&[u8; 32]>,
) -> ExchangeResult<Option<Outcome>> {
    let Some((outcome, stored_binding)) = read(connection, scope, client, request)? else {
        return Ok(None);
    };
    if outcome.request_digest() != digest || stored_binding.as_ref() != binding {
        return Err(ProtocolError::new(
            FailureKind::RequestIdentityMismatch,
            "request identity has different normalized intent or private context",
        )
        .into());
    }
    Ok(Some(outcome))
}

type StoredOutcome = (Outcome, Option<[u8; 32]>);

fn read(
    connection: &Connection,
    scope: Scope,
    client: ClientId,
    request: RequestId,
) -> ExchangeResult<Option<StoredOutcome>> {
    validate_scope(connection, scope)?;
    let stored = connection
        .query_row(
            "SELECT request_digest, capability_binding, outcome FROM exchange_outcomes
         WHERE history_id = ?1 AND client_id = ?2 AND request_id = ?3",
            params![
                scope.history().as_bytes().as_slice(),
                client.as_bytes().as_slice(),
                request.as_bytes().as_slice()
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite_error("look up submission outcome"))?;
    let Some((digest, binding, bytes)) = stored else {
        return Ok(None);
    };
    let binding = binding
        .map(|bytes| {
            bytes
                .try_into()
                .map_err(|_| Error::new(ErrorKind::Storage, "invalid stored capability binding"))
        })
        .transpose()?;
    let outcome = Document::parse(&bytes, Outcome::limits())
        .and_then(|document| Outcome::from_document(&document))
        .map_err(|_| Error::new(ErrorKind::Storage, "invalid stored submission outcome"))?;
    if outcome.scope() != scope
        || outcome.client() != client
        || outcome.request() != request
        || digest != outcome.request_digest().as_bytes()
    {
        return Err(Error::new(ErrorKind::Storage, "stored submission identity differs").into());
    }
    Ok(Some((outcome, binding)))
}

pub(crate) fn persist(
    connection: &Connection,
    outcome: &Outcome,
    binding: Option<&[u8; 32]>,
) -> postproject_core::Result<()> {
    let bytes = outcome
        .document()
        .and_then(|document| document.canonical_bytes())
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode submission outcome"))?;
    connection.execute(
        "INSERT INTO exchange_outcomes (history_id, client_id, request_id, request_digest, outcome, capability_binding)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![outcome.scope().history().as_bytes().as_slice(), outcome.client().as_bytes().as_slice(), outcome.request().as_bytes().as_slice(), outcome.request_digest().as_bytes().as_slice(), bytes, binding.map(<[u8; 32]>::as_slice)],
    ).map_err(sqlite_error("retain submission outcome"))?;
    Ok(())
}

impl SqliteProduction {
    /// Looks up a public terminal outcome by its complete scoped identity.
    ///
    /// This never returns or recreates owning worker credentials.
    ///
    /// # Errors
    /// Rejects another history/production or corrupt stored recovery state;
    /// returns persistence failures without inventing a terminal outcome.
    pub fn submission_outcome(
        &self,
        scope: Scope,
        client: ClientId,
        request: RequestId,
    ) -> ExchangeResult<Option<Outcome>> {
        lookup(&self.connection, scope, client, request)
    }
}
