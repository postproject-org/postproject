//! Limits owned rows before decoding a materialized collection.

use postproject_core::{Error, ErrorKind, Result};
use rusqlite::{Row, types::ValueRef};

pub(crate) const MAX_ROWS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct ReadBudget {
    rows: usize,
    bytes: usize,
}

impl ReadBudget {
    pub(crate) fn record(&mut self, bytes: usize) -> Result<()> {
        self.rows = self.rows.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.rows > MAX_ROWS || self.bytes > MAX_BYTES {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "materialized read exceeds 100000 rows or 64 MiB; use a bounded query",
            ));
        }
        Ok(())
    }
}

/// Checks borrowed SQLite values before the decoder copies them.
pub(crate) fn bounded<T>(
    mut decode: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> impl FnMut(&Row<'_>) -> rusqlite::Result<T> {
    let mut budget = ReadBudget::default();
    move |row| {
        let mut bytes = 0_usize;
        for column in 0..row.as_ref().column_count() {
            let size = match row.get_ref(column)? {
                ValueRef::Null => 0,
                ValueRef::Integer(_) | ValueRef::Real(_) => 8,
                ValueRef::Text(value) | ValueRef::Blob(value) => value.len(),
            };
            bytes = bytes.saturating_add(size);
        }
        budget.record(bytes).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Null,
                Box::new(ReadLimit(error)),
            )
        })?;
        decode(row)
    }
}

#[derive(Debug)]
struct ReadLimit(Error);

impl std::fmt::Display for ReadLimit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ReadLimit {}

pub(crate) fn limit_error(error: &rusqlite::Error) -> Option<Error> {
    if let rusqlite::Error::FromSqlConversionFailure(_, _, source) = error {
        source
            .downcast_ref::<ReadLimit>()
            .map(|limit| limit.0.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn oversized_stream_stops_before_copying_the_next_value() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let mut statement = connection
            .prepare("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<8) SELECT zeroblob(9*1024*1024) FROM n")
            .unwrap();
        let decoded = Cell::new(0);
        let mut rows = statement
            .query_map(
                [],
                bounded(|row| {
                    decoded.set(decoded.get() + 1);
                    row.get::<_, Vec<u8>>(0)
                }),
            )
            .unwrap();
        for _ in 0..7 {
            assert_eq!(rows.next().unwrap().unwrap().len(), 9 * 1024 * 1024);
        }
        let error = crate::sqlite_error("read oversized stream")(rows.next().unwrap().unwrap_err());
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(decoded.get(), 7);
    }

    #[test]
    fn row_limit_also_applies_to_small_values() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        let mut statement = connection
            .prepare("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100001) SELECT x FROM n")
            .unwrap();
        let mut rows = statement
            .query_map([], bounded(|row| row.get::<_, i64>(0)))
            .unwrap();
        for expected in 1..=MAX_ROWS {
            assert_eq!(
                rows.next().unwrap().unwrap(),
                i64::try_from(expected).unwrap()
            );
        }
        let error = crate::sqlite_error("read oversized stream")(rows.next().unwrap().unwrap_err());
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }
}
