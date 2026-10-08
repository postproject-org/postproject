//! Complete structured conflict details without backend keys or error strings.

use postproject_core::{
    DecisionBase, ExternalIdentifier, IdentifierScheme, ProductionId, ResourceFingerprint,
    SemanticConflictKey, TransactionConflict,
};
use serde_json::{Value, json};

use crate::{
    Document, Result,
    command::{decode_property, encode_property},
    fields::{
        checked, decode_reference, encode_reference, exact, malformed, nullable, object, text,
        unsupported,
    },
};

/// Encodes exact optimistic conflict details without private database keys.
///
/// # Errors
/// Rejects contradictory revision boundaries or unsupported key variants.
pub fn encode_transaction_conflict(conflict: &TransactionConflict) -> Result<Document> {
    Ok(Document {
        value: encode(conflict)?,
    })
}

/// Decodes complete machine-readable conflict details.
///
/// # Errors
/// Rejects malformed keys, integer fields or inconsistent revision boundaries.
pub fn decode_transaction_conflict(document: &Document) -> Result<TransactionConflict> {
    decode(&document.value)
}

pub(crate) fn encode(conflict: &TransactionConflict) -> Result<Value> {
    validate(conflict)?;
    Ok(json!({
        "key":encode_key(conflict.key())?,
        "base_revision":conflict.base_revision().map(|id| id.to_string()),
        "base_sequence":conflict.base_sequence().to_string(),
        "superseding_revision":conflict.superseding_revision().to_string(),
        "superseding_sequence":conflict.superseding_sequence().to_string()
    }))
}

pub(crate) fn decode(value: &Value) -> Result<TransactionConflict> {
    let fields = object(
        value,
        &[
            "key",
            "base_revision",
            "base_sequence",
            "superseding_revision",
            "superseding_sequence",
        ],
    )?;
    let conflict = TransactionConflict::new(
        decode_key(&fields["key"])?,
        nullable(&fields["base_revision"], exact)?,
        exact(&fields["base_sequence"])?,
        exact(&fields["superseding_revision"])?,
        exact(&fields["superseding_sequence"])?,
    );
    validate(&conflict)?;
    Ok(conflict)
}

fn validate(conflict: &TransactionConflict) -> Result<()> {
    checked(DecisionBase::new(
        ProductionId::from_bytes([0; 16]),
        conflict.base_revision(),
        conflict.base_sequence(),
    ))?;
    if conflict.superseding_sequence() <= conflict.base_sequence()
        || conflict.superseding_sequence() > i64::MAX as u64
    {
        return Err(malformed());
    }
    Ok(())
}

fn encode_identifier(identifier: &ExternalIdentifier) -> Value {
    json!({"scheme":identifier.scheme().as_str(),"value":identifier.value(),"qualifier":identifier.qualifier()})
}

fn decode_identifier(value: &Value) -> Result<ExternalIdentifier> {
    let fields = object(value, &["scheme", "value", "qualifier"])?;
    checked(ExternalIdentifier::new(
        checked(IdentifierScheme::new(text(&fields["scheme"])?))?,
        text(&fields["value"])?,
        nullable(&fields["qualifier"], |v| Ok(text(v)?.to_owned()))?,
    ))
}

fn encode_key(key: &SemanticConflictKey) -> Result<Value> {
    let kind = key.kind().as_str();
    Ok(match key {
        SemanticConflictKey::LocatorSet(id) | SemanticConflictKey::ResourceFileFacts(id) => {
            json!({"kind":kind,"resource":id.to_string()})
        }
        SemanticConflictKey::DependencySet(id) => {
            json!({"kind":kind,"representation":id.to_string()})
        }
        SemanticConflictKey::MediaRoot(id) => json!({"kind":kind,"root":id.to_string()}),
        SemanticConflictKey::MetadataProperty { target, property } => {
            json!({"kind":kind,"target":encode_reference(*target)?,"property":encode_property(property)})
        }
        SemanticConflictKey::ExternalIdentifier { target, identifier } => {
            json!({"kind":kind,"target":encode_reference(*target)?,"identifier":encode_identifier(identifier)})
        }
        SemanticConflictKey::ResourceFingerprint {
            resource_id,
            algorithm,
            version,
        } => {
            validate_domain(algorithm, *version)?;
            json!({"kind":kind,"resource":resource_id.to_string(),"algorithm":algorithm,"version":version.to_string()})
        }
        SemanticConflictKey::RepresentationFingerprint {
            representation_id,
            algorithm,
            version,
        } => {
            validate_domain(algorithm, *version)?;
            json!({"kind":kind,"representation":representation_id.to_string(),"algorithm":algorithm,"version":version.to_string()})
        }
        _ => return Err(unsupported()),
    })
}

fn validate_domain(algorithm: &str, version: u16) -> Result<()> {
    checked(ResourceFingerprint::new(algorithm, version, vec![0])).map(|_| ())
}

fn decode_key(value: &Value) -> Result<SemanticConflictKey> {
    let kind = text(value.get("kind").ok_or_else(malformed)?)?;
    Ok(match kind {
        "locator_set" | "resource_file_facts" => {
            let fields = object(value, &["kind", "resource"])?;
            let id = exact(&fields["resource"])?;
            if kind == "locator_set" {
                SemanticConflictKey::LocatorSet(id)
            } else {
                SemanticConflictKey::ResourceFileFacts(id)
            }
        }
        "dependency_set" => SemanticConflictKey::DependencySet(exact(
            &object(value, &["kind", "representation"])?["representation"],
        )?),
        "media_root" => {
            SemanticConflictKey::MediaRoot(exact(&object(value, &["kind", "root"])?["root"])?)
        }
        "metadata_property" => {
            let fields = object(value, &["kind", "target", "property"])?;
            SemanticConflictKey::MetadataProperty {
                target: decode_reference(&fields["target"])?,
                property: decode_property(&fields["property"])?,
            }
        }
        "external_identifier" => {
            let fields = object(value, &["kind", "target", "identifier"])?;
            SemanticConflictKey::ExternalIdentifier {
                target: decode_reference(&fields["target"])?,
                identifier: decode_identifier(&fields["identifier"])?,
            }
        }
        "resource_fingerprint" | "representation_fingerprint" => {
            let id_key = if kind == "resource_fingerprint" {
                "resource"
            } else {
                "representation"
            };
            let fields = object(value, &["kind", id_key, "algorithm", "version"])?;
            let algorithm = text(&fields["algorithm"])?.to_owned();
            let version = exact(&fields["version"])?;
            validate_domain(&algorithm, version)?;
            if kind == "resource_fingerprint" {
                SemanticConflictKey::ResourceFingerprint {
                    resource_id: exact(&fields[id_key])?,
                    algorithm,
                    version,
                }
            } else {
                SemanticConflictKey::RepresentationFingerprint {
                    representation_id: exact(&fields[id_key])?,
                    algorithm,
                    version,
                }
            }
        }
        _ => return Err(unsupported()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use postproject_core::{
        MediaRootId, MetadataProperty, ObjectRef, PropertyId, RepresentationId, ResourceId,
        RevisionId, VocabularyId,
    };

    #[test]
    fn all_semantic_keys_retain_exact_detail() {
        let keys = [
            SemanticConflictKey::LocatorSet(ResourceId::new()),
            SemanticConflictKey::ResourceFileFacts(ResourceId::new()),
            SemanticConflictKey::DependencySet(RepresentationId::new()),
            SemanticConflictKey::MediaRoot(MediaRootId::new()),
            SemanticConflictKey::MetadataProperty {
                target: ObjectRef::Production(ProductionId::new()),
                property: MetadataProperty::new(
                    VocabularyId::new("urn:unknown").unwrap(),
                    PropertyId::new("opaque").unwrap(),
                ),
            },
            SemanticConflictKey::ExternalIdentifier {
                target: ObjectRef::Resource(ResourceId::new()),
                identifier: ExternalIdentifier::new(
                    IdentifierScheme::new("Unknown:SCHEME").unwrap(),
                    "Exact value",
                    Some("opaque qualifier".into()),
                )
                .unwrap(),
            },
            SemanticConflictKey::ResourceFingerprint {
                resource_id: ResourceId::new(),
                algorithm: "unknown-domain".into(),
                version: 65535,
            },
            SemanticConflictKey::RepresentationFingerprint {
                representation_id: RepresentationId::new(),
                algorithm: "blake3".into(),
                version: 1,
            },
        ];
        for key in keys {
            let conflict = TransactionConflict::new(key, None, 0, RevisionId::new(), 1);
            assert_eq!(decode(&encode(&conflict).unwrap()).unwrap(), conflict);
        }
        let invalid = TransactionConflict::new(
            SemanticConflictKey::LocatorSet(ResourceId::new()),
            Some(RevisionId::new()),
            0,
            RevisionId::new(),
            1,
        );
        assert!(encode(&invalid).is_err());
    }
}
