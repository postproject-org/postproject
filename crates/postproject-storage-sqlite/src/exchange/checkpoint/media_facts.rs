//! Point reads of scalar facts and one bounded content aggregate. Fingerprint,
//! locator and metadata collections remain independent streams.

mod structure;
#[cfg(test)]
mod tests;

pub(super) use structure::content;

use postproject_core::{Asset, AssetId, RepresentationId, Resource, ResourceId, Timestamp};
use postproject_protocol::{RepresentationHeader, ResourceHeader};
use rusqlite::Connection;

use crate::{ExchangeResult, sqlite_error};

pub(super) fn asset(connection: &Connection, id: AssetId) -> ExchangeResult<Asset> {
    let (created, name, source) = connection
        .query_row(
            "SELECT created_at_micros, display_name, import_source FROM assets WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(sqlite_error("read checkpoint asset facts"))?;
    Ok(Asset::new(
        id,
        Timestamp::from_unix_micros(created),
        name,
        source,
    ))
}

pub(super) fn resource(connection: &Connection, id: ResourceId) -> ExchangeResult<ResourceHeader> {
    let (size, modified) = connection
        .query_row(
            "SELECT file_size_bytes, modified_at_micros FROM resources WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .map_err(sqlite_error("read checkpoint resource facts"))?;
    Ok(ResourceHeader::from_resource(&Resource::new(
        id,
        Vec::new(),
        crate::decode_file_facts(size, modified)?,
    )))
}

pub(super) fn representation(
    connection: &Connection,
    id: RepresentationId,
) -> ExchangeResult<RepresentationHeader> {
    let (asset, kind) = connection
        .query_row(
            "SELECT asset_id, kind FROM representations WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(sqlite_error("read checkpoint representation facts"))?;
    Ok(RepresentationHeader::new(
        id,
        AssetId::from_bytes(crate::id_bytes(asset, "owning asset")?),
        crate::decode_representation_kind(kind)?,
    )?)
}
