use crate::{ChunkSummary, Result, fields::malformed};

/// Required portable sections in deterministic wire order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointSection {
    /// Source production identity and original header facts.
    Production,
    /// Logical asset facts.
    Assets,
    /// Resource facts.
    Resources,
    /// Representations and ordered resource membership.
    Representations,
    /// Compact sequences, spans and packages.
    Structures,
    /// Logical root configuration, excluding local mappings.
    Roots,
    /// Exact locators, availability and sequence naming.
    Locators,
    /// Exact external identifiers and host bindings.
    Identifiers,
    /// Ordered current assertions.
    Metadata,
    /// Activities, participants, edges and recorded snapshots.
    Activities,
    /// Complete dependency sets and extraction observations.
    Dependencies,
    /// Current/history fingerprints and recomputation evidence.
    Fingerprints,
    /// Public inert job observations and input boundaries.
    Jobs,
    /// Original observation revision headers.
    Revisions,
    /// Original ordered observation events.
    Events,
    /// Semantic property versions using domain keys.
    ConflictVersions,
    /// Original conflict migration baseline, including genesis.
    ConflictFloor,
    /// Complete retained records, without private submission outcomes.
    Records,
    /// Earlier bounded evidence, explicitly outside the replayable suffix.
    Archives,
}

impl CheckpointSection {
    /// Every required section, including those with no facts at this head.
    pub const ALL: [Self; 19] = [
        Self::Production,
        Self::Assets,
        Self::Resources,
        Self::Representations,
        Self::Structures,
        Self::Roots,
        Self::Locators,
        Self::Identifiers,
        Self::Metadata,
        Self::Activities,
        Self::Dependencies,
        Self::Fingerprints,
        Self::Jobs,
        Self::Revisions,
        Self::Events,
        Self::ConflictVersions,
        Self::ConflictFloor,
        Self::Records,
        Self::Archives,
    ];

    /// Returns the exact wire section identity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Assets => "assets",
            Self::Resources => "resources",
            Self::Representations => "representations",
            Self::Structures => "structures",
            Self::Roots => "roots",
            Self::Locators => "locators",
            Self::Identifiers => "identifiers",
            Self::Metadata => "metadata",
            Self::Activities => "activities",
            Self::Dependencies => "dependencies",
            Self::Fingerprints => "fingerprints",
            Self::Jobs => "jobs",
            Self::Revisions => "revisions",
            Self::Events => "events",
            Self::ConflictVersions => "conflict_versions",
            Self::ConflictFloor => "conflict_floor",
            Self::Records => "records",
            Self::Archives => "archives",
        }
    }
}

/// Complete item count and bounded-chunk commitment for one required section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionSummary {
    section: CheckpointSection,
    items: u64,
    chunks: Option<ChunkSummary>,
}

impl SectionSummary {
    /// Constructs an explicit empty section or a committed nonempty section.
    ///
    /// # Errors
    /// Rejects contradictory emptiness, out-of-range counts and a body too short
    /// to contain the declared items. Production and conflict floor each need
    /// exactly one item, including in an otherwise empty production.
    pub fn new(
        section: CheckpointSection,
        items: u64,
        chunks: Option<ChunkSummary>,
    ) -> Result<Self> {
        if i64::try_from(items).is_err()
            || (items == 0) != chunks.is_none()
            || chunks
                .is_some_and(|chunks| u128::from(chunks.payload_bytes()) < u128::from(items) * 10)
            || matches!(
                section,
                CheckpointSection::Production | CheckpointSection::ConflictFloor
            ) && items != 1
        {
            return Err(malformed());
        }
        Ok(Self {
            section,
            items,
            chunks,
        })
    }

    /// Returns the required section identity.
    #[must_use]
    pub const fn section(self) -> CheckpointSection {
        self.section
    }
    /// Returns the exact count of section-defined semantic items.
    #[must_use]
    pub const fn items(self) -> u64 {
        self.items
    }
    /// Returns its complete chain commitment, absent only for an empty section.
    #[must_use]
    pub const fn chunks(self) -> Option<ChunkSummary> {
        self.chunks
    }
}
