//! Limits owned rows before decoding a materialized collection.

use postproject_core::{Error, ErrorKind, Result};

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
