"""Run the Python external-identifier and typed-metadata listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python knowledge.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
from pathlib import Path

from postproject import (
    AssetId,
    AssetRef,
    ExternalIdentifier,
    Fingerprint,
    LocatorIdentity,
    MetadataAssertion,
    MetadataBool,
    MetadataBytes,
    MetadataDecimal,
    MetadataI64,
    MetadataLanguageString,
    MetadataList,
    MetadataProperty,
    MetadataRational,
    MetadataReference,
    MetadataString,
    MetadataStruct,
    MetadataStructField,
    MetadataTimestamp,
    MetadataU64,
    MetadataUri,
    MetadataValue,
    ObjectReference,
    Production,
    RepresentationId,
    RepresentationRef,
    file_locator,
    fingerprint_file,
)

EDITORIAL = "https://example.com/ns/editorial/1"


# [known-media-adoption]
def find_known_media(
    production_path: Path, media_path: Path, fingerprint: Fingerprint
) -> AssetId:
    # A second host or process opens the same production explicitly.
    with Production.open(production_path) as production:
        locator = LocatorIdentity(file_locator(media_path))
        by_locator = production.find_known_media_by_locator(locator, limit=100)
        by_content = production.find_known_media_by_fingerprint(fingerprint, limit=100)

        # Several results are candidates for the host to present, never a winner.
        assert by_locator.items == by_content.items
        assert len(by_locator.items) == 1
        return by_locator.items[0].asset_id


# [/known-media-adoption]


# [remove-identifier]
def replace_tape_identifier(
    production: Production, asset_id: AssetId
) -> tuple[ExternalIdentifier, ...]:
    serial = ExternalIdentifier("com.example.camera.serial", "A-0007")
    tape = ExternalIdentifier("com.example.tape", "T-0012", qualifier="reel")
    with production.transaction() as transaction:
        transaction.add_external_identifier(AssetRef(asset_id), serial)
        transaction.add_external_identifier(AssetRef(asset_id), tape)
        transaction.commit()

    for identifier in production.external_identifiers[AssetRef(asset_id)]:
        print(
            f"{identifier.scheme}: {identifier.value} ({identifier.qualifier or '-'})"
        )
    # A qualifier restricts the lookup to identifiers with exactly that qualifier.
    key = (tape.scheme, tape.value, "reel")
    for target in production.objects_by_external_identifier[key]:
        print(f"tape {tape.value} identifies {target}")

    # Removal matches the exact scheme, value, and qualifier.
    with production.transaction() as transaction:
        transaction.remove_external_identifier(AssetRef(asset_id), tape)
        transaction.commit()
    return production.external_identifiers[AssetRef(asset_id)]


# [/remove-identifier]


# [typed-metadata]
def add_editorial_metadata(
    production: Production, asset_id: AssetId, original_id: RepresentationId
) -> None:
    values: dict[str, MetadataValue] = {
        "title": MetadataString("Interview"),
        "headline": MetadataLanguageString("Gespräch am Morgen", "de"),
        "reel-offset": MetadataI64(-48),
        "take-count": MetadataU64(3),
        "gain-db": MetadataDecimal(coefficient=-125, scale=1),  # -12.5
        "approved": MetadataBool(True),
        "shot-at": MetadataTimestamp(1_700_000_000_000_000),
        "script": MetadataUri("https://example.com/scripts/ep1"),
        "thumbnail": MetadataBytes(b"\x89PNG"),
        "frame-rate": MetadataRational(24_000, 1_001),
        "keywords": MetadataList(
            (MetadataString("interview"), MetadataString("exterior"))
        ),
        "lens": MetadataStruct(
            (
                MetadataStructField("model", MetadataString("Example 35mm")),
                MetadataStructField("focal-length-mm", MetadataU64(35)),
            )
        ),
        "selected-take": MetadataReference(RepresentationRef(original_id)),
    }
    with production.transaction() as transaction:
        for name, value in values.items():
            transaction.add_metadata(
                AssetRef(asset_id), MetadataProperty(EDITORIAL, name), value
            )
        transaction.commit()


def describe(value: MetadataValue) -> str:
    match value:
        case MetadataString(text) | MetadataUri(text):
            return text
        case MetadataLanguageString(text, language):
            return f"{text} @{language}"
        case MetadataI64(number) | MetadataU64(number):
            return str(number)
        case MetadataDecimal(coefficient, scale):
            return f"{coefficient}e-{scale}"
        case MetadataBool(flag):
            return "yes" if flag else "no"
        case MetadataTimestamp(unix_micros):
            return f"{unix_micros} µs since the Unix epoch"
        case MetadataBytes(data):
            return data.hex()
        case MetadataRational(numerator, denominator):
            return f"{numerator}/{denominator}"
        case MetadataList(items):
            return "[" + ", ".join(describe(item) for item in items) + "]"
        case MetadataStruct(fields):
            return (
                "{" + ", ".join(f"{f.name}: {describe(f.value)}" for f in fields) + "}"
            )
        case MetadataReference(target):
            return f"-> {target}"


def print_metadata(production: Production, target: ObjectReference) -> None:
    for assertion in production.metadata[target]:
        print(f"{assertion.property.property} = {describe(assertion.value)}")


def find_approved(production: Production) -> list[ObjectReference]:
    approved = MetadataProperty(EDITORIAL, "approved")
    targets: list[ObjectReference] = []
    cursor = None
    while True:
        # Pass the same property, value, and limit with each cursor.
        page = production.query_metadata(
            approved, limit=1, cursor=cursor, value=MetadataBool(True)
        )
        targets.extend(assertion.target for assertion in page.items)
        cursor = page.next_cursor
        if cursor is None:
            return targets


# [/typed-metadata]


# [remove-metadata]
def clear_keywords(production: Production, asset_id: AssetId) -> None:
    keywords = MetadataProperty(EDITORIAL, "keywords")
    # Removes every value of the property on this target in one change.
    with production.read_session() as view, view.edit() as transaction:
        transaction.remove_metadata_property(AssetRef(asset_id), keywords)
        transaction.commit()

    assert production.metadata_by_property[keywords] == ()


# [/remove-metadata]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: knowledge.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    production_path = work / "knowledge.pproj"
    media_path = work / "rushes" / "A001.mov"

    with Production.create(production_path, "Knowledge") as production:
        with production.transaction() as transaction:
            asset_id = transaction.import_media(media_path)
            transaction.commit()
        original_id = production.representations[asset_id][0].id
        assert (
            find_known_media(production_path, media_path, fingerprint_file(media_path))
            == asset_id
        )

        remaining = replace_tape_identifier(production, asset_id)
        assert remaining == (ExternalIdentifier("com.example.camera.serial", "A-0007"),)
        assert (
            production.objects_by_external_identifier["com.example.tape", "T-0012"]
            == ()
        )
        assert production.objects_by_external_identifier[
            "com.example.camera.serial", "A-0007"
        ] == (AssetRef(asset_id),)

        add_editorial_metadata(production, asset_id, original_id)
        print_metadata(production, AssetRef(asset_id))
        stored = {
            assertion.property.property: assertion.value
            for assertion in production.metadata[AssetRef(asset_id)]
        }
        assert len(stored) == 13
        assert stored["gain-db"] == MetadataDecimal(-125, 1)
        assert stored["frame-rate"] == MetadataRational(24_000, 1_001)
        assert stored["selected-take"] == MetadataReference(
            RepresentationRef(original_id)
        )
        assert stored["lens"] == MetadataStruct(
            (
                MetadataStructField("model", MetadataString("Example 35mm")),
                MetadataStructField("focal-length-mm", MetadataU64(35)),
            )
        )
        assert describe(stored["keywords"]) == "[interview, exterior]"

        approved = MetadataProperty(EDITORIAL, "approved")
        with production.transaction() as transaction:
            transaction.add_metadata(
                RepresentationRef(original_id), approved, MetadataBool(True)
            )
            transaction.commit()
        first_page = production.query_metadata(
            approved, limit=1, value=MetadataBool(True)
        )
        assert first_page.next_cursor is not None
        assert set(find_approved(production)) == {
            AssetRef(asset_id),
            RepresentationRef(original_id),
        }

        keywords = MetadataProperty(EDITORIAL, "keywords")
        assert len(production.metadata_by_property[keywords]) == 1
        clear_keywords(production, asset_id)
        assert all(
            assertion.property != keywords
            for assertion in production.metadata[AssetRef(asset_id)]
        )
        assert (
            MetadataAssertion(AssetRef(asset_id), approved, MetadataBool(True))
            in (production.metadata[AssetRef(asset_id)])
        )


if __name__ == "__main__":
    main()
