use postproject_protocol::{Document, IdentifierAttachment, decode_locator};
use rusqlite::params;

use crate::{ExchangeResult, encode_metadata_target, transaction::persist_locator};

use super::{
    Bodies,
    media::{insert, structural},
};

impl Bodies<'_, '_> {
    pub(super) fn locator(&self, document: &Document) -> ExchangeResult<()> {
        structural(persist_locator(
            self.transaction,
            &decode_locator(document)?,
        ))
    }

    pub(super) fn identifier(&self, document: &Document) -> ExchangeResult<()> {
        let attachment = IdentifierAttachment::from_document(document)?;
        let target = attachment.target();
        self.target_scope(target)?;
        let (kind, id) = encode_metadata_target(&target)?;
        let identifier = attachment.identifier();
        insert(self.transaction.execute("INSERT INTO external_identifiers (target_kind, target_id, scheme, value, qualifier) VALUES (?1, ?2, ?3, ?4, ?5)", params![kind, id.as_slice(), identifier.scheme().as_str(), identifier.value(), identifier.qualifier()]))?;
        Ok(())
    }
}
