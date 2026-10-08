use postproject_core::{Error, ErrorKind};
use postproject_protocol::{
    ClientId, Document, FailureKind, Limits, Outcome, ProtocolError, RequestId, Scope,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::ExchangeResult;
use crate::{SqliteProduction, sqlite_error};

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
    // No supported command in this slice uses a credential. Never recover a
    // capability-bearing request by treating its private binding as absent.
    if binding.is_some() {
        return Err(Error::new(ErrorKind::Storage, "unsupported stored capability binding").into());
    }
    let limits = Limits::new(128 * 1024, 192, 8192)?;
    let outcome = Document::parse(&bytes, limits)
        .and_then(|document| Outcome::from_document(&document))
        .map_err(|_| Error::new(ErrorKind::Storage, "invalid stored submission outcome"))?;
    if outcome.scope() != scope
        || outcome.client() != client
        || outcome.request() != request
        || digest != outcome.request_digest().as_bytes()
    {
        return Err(Error::new(ErrorKind::Storage, "stored submission identity differs").into());
    }
    Ok(Some(outcome))
}

pub(crate) fn persist(connection: &Connection, outcome: &Outcome) -> postproject_core::Result<()> {
    let bytes = outcome
        .document()
        .and_then(|document| document.canonical_bytes())
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode submission outcome"))?;
    connection.execute(
        "INSERT INTO exchange_outcomes (history_id, client_id, request_id, request_digest, outcome)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![outcome.scope().history().as_bytes().as_slice(), outcome.client().as_bytes().as_slice(), outcome.request().as_bytes().as_slice(), outcome.request_digest().as_bytes().as_slice(), bytes],
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
