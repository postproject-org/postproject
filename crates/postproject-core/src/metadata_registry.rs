//! Optional hints for a small set of externally defined metadata properties.

mod catalog;

use crate::{
    Error, ErrorKind, MetadataProperty, MetadataValue, MetadataValueKind, Result, VocabularyId,
};

pub use catalog::{
    DUBLIN_CORE_ELEMENTS_VOCABULARY, EBUCORE_VOCABULARY, IPTC_VMH_JSON_VOCABULARY,
    METADATA_VOCABULARIES, POSTPROJECT_METADATA_VOCABULARY, XMP_BASIC_VOCABULARY,
};

/// Whether a property accepts at most one assertion or repeated assertions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MetadataCardinality {
    /// Zero or one assertion on a target.
    Single,
    /// Zero or more assertions on a target.
    Repeatable,
}

/// One externally defined spelling corresponding to a property.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MetadataPropertyAlias {
    profile: &'static str,
    property: &'static str,
}

impl MetadataPropertyAlias {
    /// Returns the mapping profile or representation name.
    #[must_use]
    pub const fn profile(self) -> &'static str {
        self.profile
    }

    /// Returns the property spelling used by that profile.
    #[must_use]
    pub const fn property(self) -> &'static str {
        self.property
    }
}

/// Advisory type, cardinality, and mapping information for one property.
#[derive(Clone, Copy, Debug)]
pub struct MetadataPropertyDefinition {
    property: &'static str,
    label: &'static str,
    description: &'static str,
    accepted_kinds: &'static [MetadataValueKind],
    cardinality: MetadataCardinality,
    aliases: &'static [MetadataPropertyAlias],
    validator: Option<fn(&MetadataValue) -> bool>,
}

impl MetadataPropertyDefinition {
    /// Returns the exact vocabulary-local property identifier.
    #[must_use]
    pub const fn property(self) -> &'static str {
        self.property
    }

    /// Returns a short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    /// Returns a concise description of the property's intended meaning.
    #[must_use]
    pub const fn description(self) -> &'static str {
        self.description
    }

    /// Returns the accepted value kinds for opt-in validation.
    #[must_use]
    pub const fn accepted_kinds(self) -> &'static [MetadataValueKind] {
        self.accepted_kinds
    }

    /// Returns the suggested assertion cardinality.
    #[must_use]
    pub const fn cardinality(self) -> MetadataCardinality {
        self.cardinality
    }

    /// Returns known spellings in external mapping profiles.
    #[must_use]
    pub const fn aliases(self) -> &'static [MetadataPropertyAlias] {
        self.aliases
    }

    /// Applies the advisory type and cardinality rules to a complete value set.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::InvalidArgument`] for an unexpected value kind or
    /// more than one value for a single-valued property.
    pub fn validate_values(self, values: &[MetadataValue]) -> Result<()> {
        if self.cardinality == MetadataCardinality::Single && values.len() > 1 {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "metadata property {} accepts at most one value",
                    self.property
                ),
            ));
        }
        if let Some(value) = values.iter().find(|value| {
            !self.accepted_kinds.contains(&value.kind())
                || self.validator.is_some_and(|validator| !validator(value))
        }) {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!(
                    "metadata property {} does not accept value kind {:?}",
                    self.property,
                    value.kind()
                ),
            ));
        }
        Ok(())
    }
}

/// Documentation and property hints for one metadata vocabulary.
#[derive(Clone, Copy, Debug)]
pub struct MetadataVocabularyDefinition {
    vocabulary: &'static str,
    label: &'static str,
    reference: &'static str,
    properties: &'static [MetadataPropertyDefinition],
}

impl MetadataVocabularyDefinition {
    /// Returns the exact persisted vocabulary identifier.
    #[must_use]
    pub const fn vocabulary(self) -> &'static str {
        self.vocabulary
    }

    /// Returns a short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    /// Returns the authoritative vocabulary or schema reference.
    #[must_use]
    pub const fn reference(self) -> &'static str {
        self.reference
    }

    /// Returns the deliberately small set of built-in property hints.
    #[must_use]
    pub const fn properties(self) -> &'static [MetadataPropertyDefinition] {
        self.properties
    }
}

/// Finds a built-in vocabulary definition by exact identifier spelling.
#[must_use]
pub fn metadata_vocabulary_definition(
    vocabulary: &VocabularyId,
) -> Option<&'static MetadataVocabularyDefinition> {
    METADATA_VOCABULARIES
        .iter()
        .find(|definition| definition.vocabulary == vocabulary.as_str())
}

/// Finds a built-in property hint by exact vocabulary and property spelling.
#[must_use]
pub fn metadata_property_definition(
    property: &MetadataProperty,
) -> Option<&'static MetadataPropertyDefinition> {
    metadata_vocabulary_definition(property.vocabulary())?
        .properties
        .iter()
        .find(|definition| definition.property == property.property().as_str())
}

#[cfg(test)]
mod tests {
    use crate::{MetadataProperty, MetadataValue, PropertyId, VocabularyId};

    use super::*;

    fn property(vocabulary: &str, property: &str) -> MetadataProperty {
        MetadataProperty::new(
            VocabularyId::new(vocabulary).expect("valid vocabulary"),
            PropertyId::new(property).expect("valid property"),
        )
    }

    #[test]
    fn vocabulary_lookup_is_exact_and_descriptive() {
        let vocabulary = VocabularyId::new(IPTC_VMH_JSON_VOCABULARY).unwrap();
        let definition = metadata_vocabulary_definition(&vocabulary).expect("known vocabulary");
        assert_eq!(definition.vocabulary(), IPTC_VMH_JSON_VOCABULARY);
        assert_eq!(definition.label(), "IPTC Video Metadata Hub 1.7 JSON");
        assert_eq!(definition.reference(), IPTC_VMH_JSON_VOCABULARY);
        assert_eq!(definition.properties().len(), 2);

        let unknown = VocabularyId::new("https://example.com/metadata").unwrap();
        assert!(metadata_vocabulary_definition(&unknown).is_none());
    }

    #[test]
    fn property_hints_expose_types_cardinality_and_mappings() {
        let title = property(IPTC_VMH_JSON_VOCABULARY, "title");
        let definition = metadata_property_definition(&title).expect("known property");
        assert_eq!(definition.label(), "Title");
        assert_ne!(definition.description(), "");
        assert_eq!(definition.cardinality(), MetadataCardinality::Single);
        assert_eq!(
            definition.accepted_kinds(),
            &[MetadataValueKind::String, MetadataValueKind::LangString]
        );
        assert!(
            definition
                .aliases()
                .iter()
                .any(|alias| alias.profile() == "XMP" && alias.property() == "dc:title")
        );

        assert!(
            metadata_property_definition(&property(IPTC_VMH_JSON_VOCABULARY, "unknown")).is_none()
        );
    }

    #[test]
    fn validation_is_opt_in_and_checks_the_complete_value_set() {
        let title = metadata_property_definition(&property(IPTC_VMH_JSON_VOCABULARY, "title"))
            .expect("known title");
        let plain = MetadataValue::string("Interview").unwrap();
        let localized = MetadataValue::language_string("Interview", "en-US").unwrap();
        assert!(title.validate_values(std::slice::from_ref(&plain)).is_ok());
        assert!(
            title
                .validate_values(std::slice::from_ref(&localized))
                .is_ok()
        );
        assert!(title.validate_values(&[plain, localized]).is_err());
        assert!(title.validate_values(&[MetadataValue::u64(42)]).is_err());

        let keywords =
            metadata_property_definition(&property(IPTC_VMH_JSON_VOCABULARY, "keywords"))
                .expect("known keywords");
        assert!(
            keywords
                .validate_values(&[
                    MetadataValue::string("interview").unwrap(),
                    MetadataValue::string("studio").unwrap(),
                ])
                .is_ok()
        );
    }

    #[test]
    fn built_in_validation_callbacks_can_add_property_rules() {
        let definition = MetadataPropertyDefinition {
            property: "example",
            label: "Example",
            description: "Test-only callback coverage.",
            accepted_kinds: &[MetadataValueKind::String],
            cardinality: MetadataCardinality::Single,
            aliases: &[],
            validator: Some(|value| value.as_string() == Some("accepted")),
        };
        assert!(
            definition
                .validate_values(&[MetadataValue::string("accepted").unwrap()])
                .is_ok()
        );
        assert!(
            definition
                .validate_values(&[MetadataValue::string("rejected").unwrap()])
                .is_err()
        );
    }
}
