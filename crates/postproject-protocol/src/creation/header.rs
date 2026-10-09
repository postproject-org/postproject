use postproject_core::{MAX_CONTENT_MEMBERS, RepresentationImport, Resource};
use serde_json::json;

use crate::{
    Document, RepresentationHeader, ResourceHeader, Result,
    fields::{exact, limit, malformed, object, text, unsupported},
};

/// Declared totals for one complete newly authored representation aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationCreationStart {
    representation: RepresentationHeader,
    resources: u64,
    locators: u64,
    fingerprints: u64,
}

impl RepresentationCreationStart {
    /// Copies scalar ownership and counts without cloning the prepared aggregate.
    ///
    /// # Errors
    /// Rejects counts outside the existing resource/storage ranges.
    pub fn from_import(import: &RepresentationImport) -> Result<Self> {
        Self::new(
            RepresentationHeader::from_representation(import.representation()),
            count(import.resources().len())?,
            count(import.locators().len())?,
            count(import.representation().fingerprints().len())?,
        )
    }

    fn new(
        representation: RepresentationHeader,
        resources: u64,
        locators: u64,
        fingerprints: u64,
    ) -> Result<Self> {
        if resources == 0 || locators < resources {
            return Err(malformed());
        }
        if resources > MAX_CONTENT_MEMBERS as u64
            || locators > i64::MAX.unsigned_abs()
            || fingerprints > i64::MAX.unsigned_abs()
        {
            return Err(limit());
        }
        Ok(Self {
            representation,
            resources,
            locators,
            fingerprints,
        })
    }

    /// Returns original scalar ownership and semantic role.
    #[must_use]
    pub const fn representation(self) -> RepresentationHeader {
        self.representation
    }

    /// Returns the number of complete resource declarations that follow.
    #[must_use]
    pub const fn resource_count(self) -> u64 {
        self.resources
    }

    /// Returns the number of following locator facts.
    #[must_use]
    pub const fn locator_count(self) -> u64 {
        self.locators
    }

    /// Returns the number of following representation fingerprint observations.
    #[must_use]
    pub const fn fingerprint_count(self) -> u64 {
        self.fingerprints
    }

    /// Encodes a bounded header; fingerprints, structure and resources follow.
    ///
    /// # Errors
    /// Rejects future representation roles without a defined wire representation.
    pub fn document(self) -> Result<Document> {
        Ok(Document {
            value: json!({"kind":"representation.creation", "representation":self.representation.document()?.value, "resource_count":self.resources.to_string(), "locator_count":self.locators.to_string(), "fingerprint_count":self.fingerprints.to_string()}),
        })
    }

    /// Decodes checked ownership and continuation totals without allocating collections.
    ///
    /// # Errors
    /// Rejects unsupported kinds, unknown fields and impossible/excessive totals.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(
            &document.value,
            &[
                "kind",
                "representation",
                "resource_count",
                "locator_count",
                "fingerprint_count",
            ],
        )?;
        if text(&fields["kind"])? != "representation.creation" {
            return Err(unsupported());
        }
        Self::new(
            RepresentationHeader::from_document(&Document {
                value: fields["representation"].clone(),
            })?,
            exact(&fields["resource_count"])?,
            exact(&fields["locator_count"])?,
            exact(&fields["fingerprint_count"])?,
        )
    }
}

/// One resource's scalar facts and exact initial fingerprint continuation count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceCreationStart {
    resource: ResourceHeader,
    fingerprints: u64,
}

impl ResourceCreationStart {
    /// Copies scalar resource facts and counts, without cloning fingerprint bytes.
    ///
    /// # Errors
    /// Rejects counts outside the current signed storage range.
    pub fn from_resource(resource: &Resource) -> Result<Self> {
        Ok(Self {
            resource: ResourceHeader::from_resource(resource),
            fingerprints: count(resource.fingerprints().len())?,
        })
    }

    /// Returns the original resource identity and measured file facts.
    #[must_use]
    pub const fn resource(self) -> ResourceHeader {
        self.resource
    }

    /// Returns the exact number of following current fingerprint observations.
    #[must_use]
    pub const fn fingerprint_count(self) -> u64 {
        self.fingerprints
    }

    /// Encodes one bounded header without a fingerprint array.
    #[must_use]
    pub fn document(self) -> Document {
        Document {
            value: json!({"kind":"resource.creation", "resource":self.resource.document().value, "fingerprint_count":self.fingerprints.to_string()}),
        }
    }

    /// Decodes original scalar facts and a bounded continuation count.
    ///
    /// # Errors
    /// Rejects unknown fields/kinds, malformed facts and excessive totals.
    pub fn from_document(document: &Document) -> Result<Self> {
        let fields = object(&document.value, &["kind", "resource", "fingerprint_count"])?;
        if text(&fields["kind"])? != "resource.creation" {
            return Err(unsupported());
        }
        let fingerprints = exact(&fields["fingerprint_count"])?;
        if fingerprints > i64::MAX.unsigned_abs() {
            return Err(limit());
        }
        Ok(Self {
            resource: ResourceHeader::from_document(&Document {
                value: fields["resource"].clone(),
            })?,
            fingerprints,
        })
    }
}

fn count(count: usize) -> Result<u64> {
    let count = u64::try_from(count).map_err(|_| limit())?;
    if count > i64::MAX.unsigned_abs() {
        return Err(limit());
    }
    Ok(count)
}
