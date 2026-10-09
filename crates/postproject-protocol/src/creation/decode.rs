use std::collections::BTreeSet;

use postproject_core::{ContentStructure, Locator, ObjectRef, ResourceId};

use super::{RepresentationCreationStart, ResourceCreationStart};
use crate::{
    Document, FingerprintObservation, FingerprintState, ResourceHeader, Result, StructureAssembler,
    StructureHeader, decode_locator, fields::malformed, fingerprint::revision_sequence,
};

/// One checked creation fact ready for private transactional staging.
///
/// Facts are provisional until the decoder finishes and its enclosing record
/// passes validation. Publishing them before that point would expose a prefix.
#[derive(Debug)]
pub enum CreationFact {
    /// Current representation evidence at the original observation boundary.
    RepresentationFingerprint(FingerprintObservation),
    /// A complete checked structure, bounded by the existing core aggregate limits.
    Structure(ContentStructure),
    /// One resource's original scalar facts, before its evidence continuations.
    Resource(ResourceHeader),
    /// Current resource evidence at the original observation boundary.
    ResourceFingerprint(FingerprintObservation),
    /// One checked locator, with naming appropriate to its content resource.
    Locator(Locator),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    RepresentationFingerprints,
    StructureHeader,
    Structure,
    Resources,
    ResourceFingerprints,
    Locators,
    Complete,
    Failed,
}

/// Checks a complete creation stream while yielding individual staging facts.
///
/// Retains at most one core content structure and sets bounded by its native
/// resource limit. Fingerprint bytes and locators are yielded individually.
/// The caller supplies transport/work/disk budgets and rolls back staged facts
/// on failure; this decoder never publishes domain state itself.
pub struct CreationDecoder {
    header: RepresentationCreationStart,
    sequence: u64,
    phase: Phase,
    remaining: u64,
    structure: Option<StructureAssembler>,
    expected: BTreeSet<ResourceId>,
    resources: BTreeSet<ResourceId>,
    located: BTreeSet<ResourceId>,
    sequence_resource: Option<ResourceId>,
    resource: Option<ResourceCreationStart>,
    locators: u64,
}

impl CreationDecoder {
    /// Starts without allocating collections from advertised totals.
    ///
    /// # Errors
    /// Rejects invalid original observation sequences.
    pub fn new(header: RepresentationCreationStart, sequence: u64) -> Result<Self> {
        Ok(Self {
            header,
            sequence: revision_sequence(sequence)?,
            phase: if header.fingerprint_count() == 0 {
                Phase::StructureHeader
            } else {
                Phase::RepresentationFingerprints
            },
            remaining: header.fingerprint_count(),
            structure: None,
            expected: BTreeSet::new(),
            resources: BTreeSet::new(),
            located: BTreeSet::new(),
            sequence_resource: None,
            resource: None,
            locators: 0,
        })
    }

    /// Checks the next continuation, optionally yielding one provisional fact.
    ///
    /// # Errors
    /// Rejects incomplete/reordered ownership, invalid domains, duplicate resources,
    /// foreign locator references, wrong naming and surplus frames. Errors are terminal.
    pub fn push(&mut self, document: &Document) -> Result<Option<CreationFact>> {
        let result = self.push_open(document);
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    fn push_open(&mut self, document: &Document) -> Result<Option<CreationFact>> {
        match self.phase {
            Phase::RepresentationFingerprints => {
                let fact = self.fingerprint(
                    document,
                    ObjectRef::Representation(self.header.representation().id()),
                )?;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.phase = Phase::StructureHeader;
                }
                Ok(Some(CreationFact::RepresentationFingerprint(fact)))
            }
            Phase::StructureHeader => {
                let header = StructureHeader::from_document(document)?;
                if header.representation_id() != self.header.representation().id() {
                    return Err(malformed());
                }
                self.remaining = u64::try_from(header.member_count() + header.exception_count())
                    .map_err(|_| malformed())?;
                self.structure = Some(StructureAssembler::new(header));
                if self.remaining == 0 {
                    self.finish_structure().map(Some)
                } else {
                    self.phase = Phase::Structure;
                    Ok(None)
                }
            }
            Phase::Structure => {
                self.structure
                    .as_mut()
                    .ok_or_else(malformed)?
                    .push(document)?;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.finish_structure().map(Some)
                } else {
                    Ok(None)
                }
            }
            Phase::Resources => {
                let resource = ResourceCreationStart::from_document(document)?;
                if !self.expected.contains(&resource.resource().id())
                    || !self.resources.insert(resource.resource().id())
                {
                    return Err(malformed());
                }
                self.remaining = resource.fingerprint_count();
                self.resource = Some(resource);
                self.phase = if self.remaining == 0 {
                    self.after_resource()
                } else {
                    Phase::ResourceFingerprints
                };
                Ok(Some(CreationFact::Resource(resource.resource())))
            }
            Phase::ResourceFingerprints => {
                let resource = self.resource.ok_or_else(malformed)?;
                let fact =
                    self.fingerprint(document, ObjectRef::Resource(resource.resource().id()))?;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.phase = self.after_resource();
                }
                Ok(Some(CreationFact::ResourceFingerprint(fact)))
            }
            Phase::Locators => {
                let locator = decode_locator(document)?;
                if !self.resources.contains(&locator.resource_id())
                    || locator.sequence_naming().is_some()
                        != (Some(locator.resource_id()) == self.sequence_resource)
                {
                    return Err(malformed());
                }
                self.located.insert(locator.resource_id());
                self.locators += 1;
                if self.locators == self.header.locator_count() {
                    self.phase = Phase::Complete;
                }
                Ok(Some(CreationFact::Locator(locator)))
            }
            Phase::Complete | Phase::Failed => Err(malformed()),
        }
    }

    fn fingerprint(
        &self,
        document: &Document,
        target: ObjectRef,
    ) -> Result<FingerprintObservation> {
        let fact = FingerprintObservation::from_document(document)?;
        if fact.target() != target
            || fact.state() != FingerprintState::Current
            || fact.snapshot().observed_revision_sequence() != Some(self.sequence)
        {
            return Err(malformed());
        }
        Ok(fact)
    }

    fn finish_structure(&mut self) -> Result<CreationFact> {
        let structure = self.structure.take().ok_or_else(malformed)?.finish()?;
        self.expected = structure.resource_ids().into_iter().collect();
        if u64::try_from(self.expected.len()).ok() != Some(self.header.resource_count()) {
            return Err(malformed());
        }
        self.sequence_resource = structure
            .image_sequence_descriptor()
            .map(postproject_core::ImageSequenceDescriptor::resource_id);
        self.phase = Phase::Resources;
        Ok(CreationFact::Structure(structure))
    }

    fn after_resource(&self) -> Phase {
        if u64::try_from(self.resources.len()).ok() == Some(self.header.resource_count()) {
            Phase::Locators
        } else {
            Phase::Resources
        }
    }

    /// Confirms exact totals and complete resource/location coverage.
    ///
    /// # Errors
    /// Rejects failed/incomplete streams and missing locations for any resource.
    pub fn finish(self) -> Result<()> {
        if !self.is_complete() {
            return Err(malformed());
        }
        Ok(())
    }

    /// Returns whether all advertised facts and full location coverage are checked.
    ///
    /// An enclosing record must still validate its remaining effects, observations
    /// and integrity commitments before any staging facts become visible.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.phase == Phase::Complete
            && self.resources == self.expected
            && self.located == self.expected
            && self.locators == self.header.locator_count()
    }
}
