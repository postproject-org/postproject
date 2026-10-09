//! Preserve legacy evidence bytes without assembling a whole replacement.

use postproject_core::{Error, ErrorKind, Result, RevisionId};
use postproject_protocol::MetadataEffect;
use rusqlite::{Connection, Statement, params};

use crate::sqlite_error;

// Internal framing stays below the untrusted-file value limit. It imposes no
// aggregate limit on an ordinary native transaction or metadata replacement.
const FRAGMENT_BYTES: usize = 1024 * 1024;

pub(crate) fn persist_metadata_effects<'a>(
    connection: &Connection,
    revision: RevisionId,
    effects: impl IntoIterator<Item = &'a MetadataEffect>,
) -> Result<()> {
    let mut insert = connection
        .prepare(
            "INSERT INTO exchange_effect_fragments
         (revision_id, effect_position, fragment_position, payload) VALUES (?1, ?2, ?3, ?4)",
        )
        .map_err(sqlite_error("prepare authored effect fragments"))?;
    for (position, effect) in effects.into_iter().enumerate() {
        let position = i64::try_from(position)
            .map_err(|_| Error::new(ErrorKind::Unsupported, "too many authored effects"))?;
        let mut fragments = Fragments {
            insert: &mut insert,
            revision,
            position,
            fragment: 0,
            buffer: Vec::with_capacity(FRAGMENT_BYTES),
        };
        for part in effect.canonical_parts().map_err(|_| encoding())? {
            fragments.bytes(&part.map_err(|_| encoding())?)?;
        }
        fragments.flush()?;
    }
    Ok(())
}

struct Fragments<'a, 'connection> {
    insert: &'a mut Statement<'connection>,
    revision: RevisionId,
    position: i64,
    fragment: i64,
    buffer: Vec<u8>,
}

impl Fragments<'_, '_> {
    fn bytes(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let count = bytes.len().min(FRAGMENT_BYTES - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.buffer.len() == FRAGMENT_BYTES {
                self.flush()?;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.insert
            .execute(params![
                self.revision.as_bytes().as_slice(),
                self.position,
                self.fragment,
                self.buffer
            ])
            .map_err(sqlite_error("persist authored effect fragment"))?;
        self.fragment = self
            .fragment
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, "too many effect fragments"))?;
        self.buffer.clear();
        Ok(())
    }
}

fn encoding() -> Error {
    Error::new(
        ErrorKind::Internal,
        "cannot encode authored metadata effect",
    )
}
