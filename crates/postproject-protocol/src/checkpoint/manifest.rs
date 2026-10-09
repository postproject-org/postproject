use serde_json::json;

use crate::{
    CheckpointId, CheckpointSection, Digest, DigestDomain, Document, Extensions, FailureKind,
    Position, ProtocolError, Result, SectionSummary,
    fields::{array, exact, malformed, nullable, object, text, unsupported},
};

/// Bounded completeness commitment from one coherent source view.
///
/// Its own digest is distinct from the source head/anchor. Decoding verifies
/// framing and completeness declarations; section bodies still need full domain
/// validation before creating a passive store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointManifest {
    id: CheckpointId,
    head: Position,
    floor: Position,
    sections: [SectionSummary; 18],
    extensions: Extensions,
}

impl CheckpointManifest {
    /// Constructs exactly one ordered descriptor for every required section.
    ///
    /// # Errors
    /// Rejects mismatched scopes, inconsistent boundaries, altered source anchors
    /// duplicate/reordered sections and incomplete retained history totals.
    /// Does not authenticate the source.
    pub fn new(
        id: CheckpointId,
        head: Position,
        floor: Position,
        sections: [SectionSummary; 18],
        extensions: Extensions,
    ) -> Result<Self> {
        if head.scope() != floor.scope() {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "checkpoint boundaries have different source scopes",
            ));
        }
        if floor.sequence() > head.sequence()
            || floor.sequence() == head.sequence() && floor != head
            || sections
                .iter()
                .zip(CheckpointSection::ALL)
                .any(|(summary, section)| summary.section() != section)
        {
            return Err(malformed());
        }
        for summary in &sections {
            let valid = match summary.section() {
                CheckpointSection::Revisions => summary.items() == head.sequence(),
                CheckpointSection::Events => summary.items() >= head.sequence(),
                CheckpointSection::Records => summary.items() == head.sequence() - floor.sequence(),
                _ => true,
            };
            if !valid {
                return Err(malformed());
            }
        }
        if Position::anchor(floor.decision_base())? != floor {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "checkpoint source floor anchor mismatch",
            ));
        }
        Ok(Self {
            id,
            head,
            floor,
            sections,
            extensions,
        })
    }

    /// Returns the identity of this export, not its source history identity.
    #[must_use]
    pub const fn id(&self) -> CheckpointId {
        self.id
    }
    /// Returns the exact source continuation boundary at the retained view.
    #[must_use]
    pub const fn head(&self) -> Position {
        self.head
    }
    /// Returns the source's original retained replay floor/anchor.
    #[must_use]
    pub const fn floor(&self) -> Position {
        self.floor
    }
    /// Returns every ordered section, including explicit empty sections.
    #[must_use]
    pub const fn sections(&self) -> &[SectionSummary; 18] {
        &self.sections
    }
    /// Returns preserved noncritical facts included in integrity.
    #[must_use]
    pub const fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Starts verification of a nonempty section; empty sections return `None`.
    #[must_use]
    pub fn section_chain(&self, section: CheckpointSection) -> Option<crate::CheckpointChunkChain> {
        self.sections
            .iter()
            .find(|summary| summary.section() == section)?
            .chunks()?;
        Some(crate::CheckpointChunkChain::new(
            self.head.scope(),
            self.id,
            section,
        ))
    }

    /// Checks complete transport integrity against one advertised section.
    ///
    /// # Errors
    /// Rejects a foreign/empty/partial/altered section chain. Section-defined
    /// item totals and domain invariants still need validation by the importer.
    pub fn verify_section(
        &self,
        section: CheckpointSection,
        chain: crate::CheckpointChunkChain,
    ) -> Result<()> {
        if !chain.belongs_to(self.head.scope(), self.id, section) {
            return Err(ProtocolError::new(
                FailureKind::ScopeMismatch,
                "section chain belongs to another checkpoint",
            ));
        }
        let declared = self
            .sections
            .iter()
            .find(|summary| summary.section() == section)
            .and_then(|summary| summary.chunks())
            .ok_or_else(malformed)?;
        if chain.finish()? != declared {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "checkpoint section chunk summary mismatch",
            ));
        }
        Ok(())
    }

    /// Encodes a bounded header with its separate manifest digest.
    ///
    /// # Errors
    /// Rejects internally unencodable extension values.
    pub fn document(&self) -> Result<Document> {
        let sections: Vec<_> = self.sections.iter().map(|summary| {
            let chunks = summary.chunks().map(|chunks| json!({"count":chunks.count().to_string(), "payload_bytes":chunks.payload_bytes().to_string(), "last_digest":chunks.last_digest().to_string()}));
            json!({"section":summary.section().as_str(), "items":summary.items().to_string(), "chunks":chunks})
        }).collect();
        let mut document = Document {
            value: json!({"kind":"checkpoint.manifest", "version":"1", "required_features":["checkpoints.v1"], "checkpoint":self.id.to_string(), "head":self.head.document().value, "floor":self.floor.document().value, "sections":sections, "extensions":self.extensions.document().value}),
        };
        document.value["digest"] = document.digest(DigestDomain::Manifest)?.to_string().into();
        Ok(document)
    }

    /// Decodes exact completeness declarations and verifies manifest integrity.
    ///
    /// # Errors
    /// Rejects unknown fields/features/sections, invalid counts or digests and
    /// incomplete declarations. A valid header is not a validated snapshot.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "version",
                "required_features",
                "checkpoint",
                "head",
                "floor",
                "sections",
                "extensions",
                "digest",
            ],
        )?;
        if text(&fields["kind"])? != "checkpoint.manifest" {
            return Err(malformed());
        }
        if text(&fields["version"])? != "1" {
            return Err(unsupported());
        }
        let features = array(&fields["required_features"], 64)?;
        if features.len() != 1 || text(&features[0])? != "checkpoints.v1" {
            return Err(unsupported());
        }
        let sections = array(&fields["sections"], CheckpointSection::ALL.len())?
            .iter()
            .map(|value| {
                let fields = object(value, &["section", "items", "chunks"])?;
                let name = text(&fields["section"])?;
                let section = CheckpointSection::ALL
                    .into_iter()
                    .find(|section| section.as_str() == name)
                    .ok_or_else(unsupported)?;
                let chunks = nullable(&fields["chunks"], |value| {
                    let fields = object(value, &["count", "payload_bytes", "last_digest"])?;
                    crate::ChunkSummary::new(
                        exact(&fields["count"])?,
                        exact(&fields["payload_bytes"])?,
                        exact(&fields["last_digest"])?,
                    )
                })?;
                SectionSummary::new(section, exact(&fields["items"])?, chunks)
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| malformed())?;
        let manifest = Self::new(
            exact(&fields["checkpoint"])?,
            Position::from_document(&Document {
                value: fields["head"].clone(),
            })?,
            Position::from_document(&Document {
                value: fields["floor"].clone(),
            })?,
            sections,
            Extensions::new(Document {
                value: fields["extensions"].clone(),
            })?,
        )?;
        let declared: Digest = exact(&fields["digest"])?;
        if document.digest(DigestDomain::Manifest)? != declared {
            return Err(ProtocolError::new(
                FailureKind::Integrity,
                "checkpoint manifest digest mismatch",
            ));
        }
        Ok(manifest)
    }
}
