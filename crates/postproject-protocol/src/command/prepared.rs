//! Complete prepared media aggregates; submission performs no media I/O.

use postproject_core::{
    MAX_CONTENT_MEMBERS, MAX_SEQUENCE_EXCEPTIONS, OriginalMediaImport, Representation,
    RepresentationFingerprint, RepresentationImport, Resource, ResourceFingerprint,
};
use serde_json::{Value, json};

use super::{Command, evidence};
use crate::{
    Document, RepresentationHeader, ResourceHeader, Result, StructureAssembler, StructureHeader,
    decode_asset, decode_locator, encode_asset, encode_locator, encode_structure,
    fields::{array, checked, malformed, object, unsupported},
};

fn document(value: &Value) -> Document {
    Document {
        value: value.clone(),
    }
}

pub(super) fn encode(command: &Command) -> Result<Value> {
    Ok(match command {
        Command::ImportOriginal(import) => json!({"kind":"media.import-original",
            "asset":encode_asset(import.asset()).value,
            "media":encode_media(import.representation(), import.resources(), import.locators())?}),
        Command::AddRepresentation(import) => json!({"kind":"representation.add",
            "media":encode_media(import.representation(), import.resources(), import.locators())?}),
        _ => return Err(unsupported()),
    })
}

fn encode_media(
    representation: &Representation,
    resources: &[Resource],
    locators: &[postproject_core::Locator],
) -> Result<Value> {
    let structure = encode_structure(representation)?
        .map(|frame| frame.map(|frame| frame.value))
        .collect::<Result<Vec<_>>>()?;
    let resources: Vec<_> = resources.iter().map(|resource| json!({
        "header":ResourceHeader::from_resource(resource).document().value,
        "fingerprints":resource.fingerprints().iter().map(|fingerprint|
            evidence::encode(fingerprint.algorithm(), fingerprint.version(), fingerprint.value()))
            .collect::<Vec<_>>()
    })).collect();
    Ok(
        json!({"header":RepresentationHeader::from_representation(representation).document()?.value,
        "structure":structure,
        "fingerprints":representation.fingerprints().iter().map(|fingerprint|
            evidence::encode(fingerprint.algorithm(), fingerprint.version(), fingerprint.value()))
            .collect::<Vec<_>>(),
        "resources":resources,
        "locators":locators.iter().map(|locator| encode_locator(locator).map(|frame| frame.value))
            .collect::<Result<Vec<_>>>()?}),
    )
}

pub(super) fn decode(kind: &str, value: &Value) -> Result<Command> {
    Ok(match kind {
        "media.import-original" => {
            let fields = object(value, &["kind", "asset", "media"])?;
            let asset = decode_asset(&document(&fields["asset"]))?;
            let (representation, resources, locators) =
                decode_media(&fields["media"])?.into_parts();
            Command::ImportOriginal(checked(OriginalMediaImport::new(
                asset,
                representation,
                resources,
                locators,
            ))?)
        }
        "representation.add" => {
            let fields = object(value, &["kind", "media"])?;
            Command::AddRepresentation(decode_media(&fields["media"])?)
        }
        _ => return Err(unsupported()),
    })
}

fn decode_media(value: &Value) -> Result<RepresentationImport> {
    let fields = object(
        value,
        &[
            "header",
            "structure",
            "fingerprints",
            "resources",
            "locators",
        ],
    )?;
    let header = RepresentationHeader::from_document(&document(&fields["header"]))?;
    let frames = array(
        &fields["structure"],
        MAX_CONTENT_MEMBERS + MAX_SEQUENCE_EXCEPTIONS + 1,
    )?;
    let structure =
        StructureHeader::from_document(&document(frames.first().ok_or_else(malformed)?))?;
    if structure.representation_id() != header.id() {
        return Err(malformed());
    }
    let mut assembler = StructureAssembler::new(structure);
    for frame in &frames[1..] {
        assembler.push(&document(frame))?;
    }
    let fingerprints = array(&fields["fingerprints"], 1_000_000)?
        .iter()
        .map(|value| {
            let (algorithm, version, bytes) = evidence::decode(value)?;
            checked(RepresentationFingerprint::new(algorithm, version, bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    let representation = Representation::new(
        header.id(),
        header.asset_id(),
        header.kind(),
        assembler.finish()?,
        fingerprints,
    );
    let resources = array(&fields["resources"], MAX_CONTENT_MEMBERS)?
        .iter()
        .map(decode_resource)
        .collect::<Result<Vec<_>>>()?;
    let locators = array(&fields["locators"], 1_000_000)?
        .iter()
        .map(|value| decode_locator(&document(value)))
        .collect::<Result<Vec<_>>>()?;
    checked(RepresentationImport::new(
        representation,
        resources,
        locators,
    ))
}

fn decode_resource(value: &Value) -> Result<Resource> {
    let fields = object(value, &["header", "fingerprints"])?;
    let header = ResourceHeader::from_document(&document(&fields["header"]))?;
    let fingerprints = array(&fields["fingerprints"], 1_000_000)?
        .iter()
        .map(|value| {
            let (algorithm, version, bytes) = evidence::decode(value)?;
            checked(ResourceFingerprint::new(algorithm, version, bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Resource::new(
        header.id(),
        fingerprints,
        header.file_facts(),
    ))
}
