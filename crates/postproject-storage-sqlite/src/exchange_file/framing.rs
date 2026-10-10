use std::io::{Read, Seek, SeekFrom, Write};

use postproject_core::{Error, ErrorKind};
use postproject_protocol::{
    CheckpointManifest, Document, FailureKind, ProtocolError, RecordManifest,
};

use super::{FileLimits, FileManifest, FileReader};
use crate::ExchangeResult;

pub(super) const CHECKPOINT: &[u8; 8] = b"PPXC\0\0\0\x01";
pub(super) const RECORD: &[u8; 8] = b"PPXR\0\0\0\x01";
pub(super) const CHANGES: &[u8; 8] = b"PPXD\0\0\0\x01";
const MANIFEST_BYTES: u64 = 512 * 1024;

pub(super) fn open<R: Read + Seek>(
    mut source: R,
    limits: FileLimits,
) -> ExchangeResult<FileReader<R>> {
    let end = source.seek(SeekFrom::End(0)).map_err(io_error)?;
    if end > limits.bytes {
        return Err(budget().into());
    }
    if end < 24 {
        return Err(invalid().into());
    }
    source.seek(SeekFrom::Start(end - 8)).map_err(io_error)?;
    let length = read_length(&mut source)?;
    let body_end = end
        .checked_sub(length)
        .and_then(|offset| offset.checked_sub(16))
        .filter(|offset| *offset >= 8)
        .ok_or_else(invalid)?;
    if length == 0 {
        return Err(invalid().into());
    }
    if length > MANIFEST_BYTES
        || length > u64::try_from(limits.document.max_bytes()).map_err(|_| budget())?
    {
        return Err(budget().into());
    }
    source.seek(SeekFrom::Start(body_end)).map_err(io_error)?;
    if read_length(&mut source)? != length {
        return Err(invalid().into());
    }
    let document = read_document(&mut source, length, limits)?;
    source.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut magic = [0; 8];
    exact(&mut source, &mut magic)?;
    let manifest = match &magic {
        CHECKPOINT => {
            FileManifest::Checkpoint(Box::new(CheckpointManifest::from_document(&document)?))
        }
        RECORD => FileManifest::Record(Box::new(RecordManifest::from_document(&document)?)),
        CHANGES => FileManifest::Changes(postproject_protocol::Position::from_document(&document)?),
        _ => {
            return Err(ProtocolError::new(
                FailureKind::Unsupported,
                "unsupported exchange file framing",
            )
            .into());
        }
    };
    Ok(FileReader {
        source,
        manifest,
        body_end,
        offset: 8,
        limits,
        failed: false,
    })
}

pub(super) fn next<R: Read>(reader: &mut FileReader<R>) -> ExchangeResult<Option<Document>> {
    if reader.offset == reader.body_end {
        return Ok(None);
    }
    if reader.body_end - reader.offset < 8 {
        return Err(invalid().into());
    }
    let length = read_length(&mut reader.source)?;
    let end = reader
        .offset
        .checked_add(8)
        .and_then(|offset| offset.checked_add(length))
        .ok_or_else(budget)?;
    if end > reader.body_end {
        return Err(invalid().into());
    }
    let document = read_document(&mut reader.source, length, reader.limits)?;
    reader.offset = end;
    Ok(Some(document))
}

fn read_document(
    source: &mut impl Read,
    length: u64,
    limits: FileLimits,
) -> ExchangeResult<Document> {
    let length = usize::try_from(length).map_err(|_| budget())?;
    if length == 0 || length > limits.document.max_bytes() {
        return Err(budget().into());
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| budget())?;
    bytes.resize(length, 0);
    exact(source, &mut bytes)?;
    let document = Document::parse(&bytes, limits.document)?;
    if document.canonical_bytes()? != bytes {
        return Err(invalid().into());
    }
    Ok(document)
}

fn read_length(source: &mut impl Read) -> ExchangeResult<u64> {
    let mut bytes = [0; 8];
    exact(source, &mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

fn exact(source: &mut impl Read, bytes: &mut [u8]) -> ExchangeResult<()> {
    source.read_exact(bytes).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            invalid().into()
        } else {
            io_error(error).into()
        }
    })
}

pub(super) fn write(writer: &mut impl Write, document: &Document) -> ExchangeResult<u64> {
    let bytes = document.canonical_bytes()?;
    let length = u64::try_from(bytes.len()).map_err(|_| budget())?;
    writer.write_all(&length.to_be_bytes()).map_err(io_error)?;
    writer.write_all(&bytes).map_err(io_error)?;
    Ok(length)
}

pub(super) fn io_error(_: std::io::Error) -> Error {
    Error::new(ErrorKind::Io, "exchange file I/O failed")
}
pub(super) fn invalid() -> ProtocolError {
    ProtocolError::new(FailureKind::Integrity, "invalid or truncated exchange file")
}
pub(super) fn budget() -> ProtocolError {
    ProtocolError::new(
        FailureKind::LimitExceeded,
        "exchange file exceeds receiver budget",
    )
}
