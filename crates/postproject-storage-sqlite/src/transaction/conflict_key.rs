//! Decode private index keys back into portable domain values, never wire bytes.

#[cfg(test)]
mod tests;

use postproject_core::{
    Error, ErrorKind, ExternalIdentifier, IdentifierScheme, MAX_IDENTIFIER_QUALIFIER_BYTES,
    MAX_IDENTIFIER_SCHEME_BYTES, MAX_IDENTIFIER_VALUE_BYTES, MediaRootId, MetadataProperty,
    ObjectRef, PropertyId, RepresentationId, ResourceFingerprint, ResourceId, Result,
    SemanticConflictKey, VocabularyId,
};

pub(crate) fn decode(encoded: &[u8]) -> Result<SemanticConflictKey> {
    const MAX_KEY_BYTES: usize = 1
        + 8
        + 16
        + 4
        + MAX_IDENTIFIER_SCHEME_BYTES
        + 4
        + MAX_IDENTIFIER_VALUE_BYTES
        + 1
        + 4
        + MAX_IDENTIFIER_QUALIFIER_BYTES;
    if encoded.len() > MAX_KEY_BYTES {
        return Err(invalid());
    }
    let mut input = Input(encoded);
    let key = match input.take::<1>()?[0] {
        1 => SemanticConflictKey::LocatorSet(ResourceId::from_bytes(input.take()?)),
        2 => SemanticConflictKey::MetadataProperty {
            target: input.target()?,
            property: MetadataProperty::new(
                VocabularyId::new(input.text()?).map_err(|_| invalid())?,
                PropertyId::new(input.text()?).map_err(|_| invalid())?,
            ),
        },
        3 => SemanticConflictKey::DependencySet(RepresentationId::from_bytes(input.take()?)),
        4 => SemanticConflictKey::MediaRoot(MediaRootId::from_bytes(input.take()?)),
        5 => {
            let target = input.target()?;
            let scheme = IdentifierScheme::new(input.text()?).map_err(|_| invalid())?;
            let value = input.text()?;
            let qualifier = match input.take::<1>()?[0] {
                0 => None,
                1 => Some(input.text()?),
                _ => return Err(invalid()),
            };
            SemanticConflictKey::ExternalIdentifier {
                target,
                identifier: ExternalIdentifier::new(scheme, value, qualifier)
                    .map_err(|_| invalid())?,
            }
        }
        domain @ (6 | 7) => {
            let id = input.take::<16>()?;
            let algorithm = input.text()?;
            let version = u16::from_be_bytes(input.take()?);
            ResourceFingerprint::new(algorithm.clone(), version, vec![1]).map_err(|_| invalid())?;
            if domain == 6 {
                SemanticConflictKey::ResourceFingerprint {
                    resource_id: ResourceId::from_bytes(id),
                    algorithm,
                    version,
                }
            } else {
                SemanticConflictKey::RepresentationFingerprint {
                    representation_id: RepresentationId::from_bytes(id),
                    algorithm,
                    version,
                }
            }
        }
        8 => SemanticConflictKey::ResourceFileFacts(ResourceId::from_bytes(input.take()?)),
        _ => return Err(invalid()),
    };
    if !input.0.is_empty() || super::encode_conflict_key(&key)? != encoded {
        return Err(invalid());
    }
    Ok(key)
}

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.0.get(..N).ok_or_else(invalid)?;
        self.0 = self.0.get(N..).ok_or_else(invalid)?;
        bytes.try_into().map_err(|_| invalid())
    }

    fn text(&mut self) -> Result<String> {
        let length = usize::try_from(u32::from_be_bytes(self.take()?)).map_err(|_| invalid())?;
        let bytes = self.0.get(..length).ok_or_else(invalid)?;
        self.0 = self.0.get(length..).ok_or_else(invalid)?;
        Ok(std::str::from_utf8(bytes)
            .map_err(|_| invalid())?
            .to_owned())
    }

    fn target(&mut self) -> Result<ObjectRef> {
        let kind = i64::from_be_bytes(self.take()?);
        crate::decode_metadata_target(kind, self.take::<16>()?.to_vec()).map_err(|_| invalid())
    }
}

fn invalid() -> Error {
    Error::new(ErrorKind::Storage, "invalid stored semantic conflict key")
}
