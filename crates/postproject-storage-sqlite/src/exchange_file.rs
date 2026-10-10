//! Bounded, seekable files for portable checkpoints and complete records.
//!
//! Writers emit chunks first and seal the file with the final manifest. Readers
//! locate that bounded manifest without loading the body. A file's integrity
//! still needs the ordinary checkpoint importer or atomic record application.

mod framing;

use std::io::{Read, Seek, Write};

use postproject_protocol::{CheckpointManifest, Document, RecordManifest};

use crate::{ExchangeResult, RecordReader, SqliteProduction};

/// Streams a pinned checkpoint and seals its manifest only after every chunk.
///
/// The caller publishes the destination only after this returns and its own
/// durable file flush succeeds. Failed writers leave an unusable partial file.
///
/// # Errors
/// Returns capture, encoding or I/O failures without a successful manifest.
pub fn write_checkpoint(
    source: &SqliteProduction,
    mut writer: impl Write,
) -> ExchangeResult<CheckpointManifest> {
    writer
        .write_all(framing::CHECKPOINT)
        .map_err(framing::io_error)?;
    let manifest = source
        .export_checkpoint(|chunk| framing::write(&mut writer, &chunk.document()?).map(|_| ()))?;
    seal(&mut writer, &manifest.document()?)?;
    Ok(manifest)
}

/// Streams one independently pinned complete record and seals its manifest.
///
/// # Errors
/// Returns history, integrity, encoding or I/O failures. Partial output cannot
/// be applied; the caller publishes the file only after success and durable flush.
pub fn write_record(
    mut source: RecordReader,
    mut writer: impl Write,
) -> ExchangeResult<RecordManifest> {
    writer
        .write_all(framing::RECORD)
        .map_err(framing::io_error)?;
    while let Some(chunk) = source.next_chunk()? {
        framing::write(&mut writer, &chunk.document()?)?;
    }
    let manifest = source.manifest().clone();
    seal(&mut writer, &manifest.document()?)?;
    Ok(manifest)
}

fn seal(writer: &mut impl Write, document: &Document) -> ExchangeResult<()> {
    let length = framing::write(writer, document)?;
    writer
        .write_all(&length.to_be_bytes())
        .map_err(framing::io_error)?;
    writer.flush().map_err(framing::io_error)?;
    Ok(())
}

/// The completeness commitment sealing one exchange file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileManifest {
    /// All portable sections at one pinned source head.
    Checkpoint(Box<CheckpointManifest>),
    /// One complete authored revision and its immediate predecessor.
    Record(Box<RecordManifest>),
}

/// Receiver-controlled file and individual envelope byte budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileLimits {
    bytes: u64,
    document: postproject_protocol::Limits,
}

impl FileLimits {
    /// Creates positive file and checked JSON envelope limits.
    ///
    /// # Errors
    /// Rejects a zero file budget. These do not limit authoritative native edits.
    pub fn new(
        bytes: u64,
        document: postproject_protocol::Limits,
    ) -> postproject_protocol::Result<Self> {
        if bytes == 0 {
            return Err(framing::budget());
        }
        Ok(Self { bytes, document })
    }
}

impl Default for FileLimits {
    fn default() -> Self {
        Self {
            bytes: 1024 * 1024 * 1024,
            document: postproject_protocol::Limits::default(),
        }
    }
}

/// Owns a bounded file reader; drop cancels without changing a production.
pub struct FileReader<R> {
    source: R,
    manifest: FileManifest,
    body_end: u64,
    offset: u64,
    limits: FileLimits,
    failed: bool,
}

impl<R: Read + Seek> FileReader<R> {
    /// Locates and validates the final manifest before reading any chunk.
    ///
    /// # Errors
    /// Rejects unsupported framing, truncation, invalid manifests and budgets.
    /// Chunk chains and domain facts are checked by the receiver separately.
    pub fn open(source: R, limits: FileLimits) -> ExchangeResult<Self> {
        framing::open(source, limits)
    }

    /// Borrows the final completeness commitment; this is not a domain audit.
    #[must_use]
    pub const fn manifest(&self) -> &FileManifest {
        &self.manifest
    }

    /// Reads the next bounded canonical chunk envelope, or `None` at the seal.
    ///
    /// # Errors
    /// Any framing, I/O or budget failure is terminal. Subsequent reads fail.
    pub fn next_document(&mut self) -> ExchangeResult<Option<Document>> {
        if self.failed {
            return Err(framing::invalid().into());
        }
        let result = framing::next(self);
        self.failed = result.is_err();
        result
    }
}
