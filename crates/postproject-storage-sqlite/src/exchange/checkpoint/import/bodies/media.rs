#[cfg(test)]
mod tests;

use postproject_core::{RepresentationId, Resource};
use postproject_protocol::{
    Document, RepresentationHeader, ResourceHeader, StructureAssembler, StructureHeader,
    decode_asset,
};
use rusqlite::params;

use crate::{
    ExchangeError, ExchangeResult,
    transaction::{
        encode_representation_kind, encode_structure_kind, persist_content_structure,
        persist_resource,
    },
};

use super::{super::super::invalid, Bodies};

pub(super) struct PendingStructure {
    owner: RepresentationId,
    assembler: StructureAssembler,
    remaining: usize,
}

impl Bodies<'_, '_> {
    pub(super) fn asset(&self, document: &Document) -> ExchangeResult<()> {
        let asset = decode_asset(document)?;
        insert(self.transaction.execute("INSERT INTO assets (id, created_at_micros, display_name, import_source) VALUES (?1, ?2, ?3, ?4)", params![asset.id().as_bytes().as_slice(), asset.created_at().as_unix_micros(), asset.display_name(), asset.import_source()]))?;
        Ok(())
    }

    pub(super) fn resource(&self, document: &Document) -> ExchangeResult<()> {
        let header = ResourceHeader::from_document(document)?;
        structural(persist_resource(
            self.transaction,
            &Resource::new(header.id(), Vec::new(), header.file_facts()),
            0,
        ))
    }

    pub(super) fn representation(&self, document: &Document) -> ExchangeResult<()> {
        let header = RepresentationHeader::from_document(document)?;
        insert(self.transaction.execute("INSERT INTO representations (id, asset_id, kind, structure_kind) VALUES (?1, ?2, ?3, 0)", params![header.id().as_bytes().as_slice(), header.asset_id().as_bytes().as_slice(), encode_representation_kind(header.kind())?]))?;
        Ok(())
    }

    pub(super) fn structure(&mut self, document: &Document) -> ExchangeResult<bool> {
        let starts = self.structure.is_none();
        let pending = if let Some(mut pending) = self.structure.take() {
            pending.assembler.push(document)?;
            pending.remaining = pending.remaining.checked_sub(1).ok_or_else(invalid)?;
            pending
        } else {
            let header = StructureHeader::from_document(document)?;
            let duplicate: bool = self.transaction.query_row("SELECT EXISTS(SELECT 1 FROM representation_resources WHERE representation_id = ?1)", [header.representation_id().as_bytes().as_slice()], |row| row.get(0))
                .map_err(crate::sqlite_error("check unique checkpoint structure"))?;
            if duplicate {
                return Err(invalid().into());
            }
            let updated = insert(self.transaction.execute(
                "UPDATE representations SET structure_kind = ?1 WHERE id = ?2",
                params![
                    encode_structure_kind(header.kind())?,
                    header.representation_id().as_bytes().as_slice()
                ],
            ))?;
            if updated != 1 {
                return Err(invalid().into());
            }
            PendingStructure {
                owner: header.representation_id(),
                remaining: header
                    .member_count()
                    .checked_add(header.exception_count())
                    .ok_or_else(invalid)?,
                assembler: StructureAssembler::new(header),
            }
        };
        if pending.remaining == 0 {
            let structure = pending.assembler.finish()?;
            structural(persist_content_structure(
                self.transaction,
                pending.owner,
                &structure,
            ))?;
        } else {
            self.structure = Some(pending);
        }
        Ok(starts)
    }
}

pub(super) fn insert<T>(result: rusqlite::Result<T>) -> ExchangeResult<T> {
    result.map_err(|error| {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
            invalid().into()
        } else {
            crate::sqlite_error("stage checkpoint media fact")(error).into()
        }
    })
}

pub(super) fn structural<T>(result: postproject_core::Result<T>) -> ExchangeResult<T> {
    result.map_err(|error| match error.kind() {
        postproject_core::ErrorKind::AlreadyExists
        | postproject_core::ErrorKind::InvalidArgument
        | postproject_core::ErrorKind::NotFound => invalid().into(),
        _ => ExchangeError::Store(error),
    })
}
