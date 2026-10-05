"""Run the Python media-structure and media-knowledge listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python media.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
from pathlib import Path

from postproject import (
    AssetId,
    AvailabilityIssue,
    AvailabilityIssueKind,
    CancelledError,
    CancelToken,
    ContentObservationOutcome,
    ContentStructureKind,
    ContentVerification,
    EvidenceKind,
    FileResourceInput,
    FileSource,
    Fingerprint,
    ImageSequenceSource,
    LocatorMatch,
    MediaRootId,
    OrderedPartsSource,
    PackageSource,
    Production,
    Representation,
    RepresentationAvailability,
    RepresentationFingerprintObservedEvent,
    RepresentationId,
    RepresentationKind,
    ResolutionEvidence,
    ResourceFingerprintObservedEvent,
    ResourceId,
    ResourceResolutionState,
    RevisionEvent,
    SequenceNaming,
    VerificationMode,
    file_locator,
    fingerprint_file,
    locator_file_path,
)


# [import-sequence]
def import_image_strip(production: Production, directory: Path) -> AssetId:
    # The sequence becomes the new asset's only original representation. The
    # directory and file naming become its locator; frames and rate its
    # descriptor.
    strip = ImageSequenceSource(
        directory,
        naming=SequenceNaming("shot010.", ".exr", 4),
        start=1001,
        end=1004,
        step=1,
        rate_numerator=24,
        rate_denominator=1,
        missing_frames=(1003,),
    )
    with production.transaction() as transaction:
        asset_id = transaction.import_media(strip, display_name="shot010 strip")
        transaction.commit()

    representations = production.representations[asset_id]
    print(f"{len(representations)} representation, {representations[0].structure_kind}")
    return asset_id


# [/import-sequence]


# [add-representation]
def add_proxy(
    production: Production, asset_id: AssetId, proxy: Path
) -> RepresentationId:
    # A plain path is a single-file source too.
    with production.transaction() as transaction:
        proxy_id = transaction.add_representation(
            asset_id, RepresentationKind.PROXY, FileSource(proxy)
        )
        transaction.commit()
    return proxy_id


# [/add-representation]


# [ordered-parts]
def add_spanned_clip(
    production: Production, asset_id: AssetId, directory: Path
) -> RepresentationId:
    # One recording spanned over several files: every part is required and
    # the member order is preserved.
    parts = OrderedPartsSource(
        (
            FileResourceInput(
                str(directory / "CLIP0001.MTS"), "org.postproject:essence"
            ),
            FileResourceInput(
                str(directory / "CLIP0002.MTS"), "org.postproject:span-part"
            ),
        )
    )
    with production.transaction() as transaction:
        clip_id = transaction.add_representation(
            asset_id, RepresentationKind.ORIGINAL, parts
        )
        transaction.commit()
    return clip_id


# [/ordered-parts]


# [package-representation]
def add_package(
    production: Production, asset_id: AssetId, directory: Path
) -> RepresentationId:
    members = PackageSource(
        (
            FileResourceInput(str(directory / "clip.mxf"), "org.postproject:essence"),
            # An optional member may go missing without making the package offline.
            FileResourceInput(
                str(directory / "clip.xml"), "org.postproject:sidecar", required=False
            ),
        )
    )
    with production.transaction() as transaction:
        package_id = transaction.add_representation(
            asset_id, RepresentationKind.OPTIMIZED, members
        )
        transaction.commit()
    return package_id


# [/package-representation]


# [representation-structure]
def describe_representations(
    production: Production, asset_id: AssetId
) -> list[Representation]:
    described: list[Representation] = []
    cursor = None
    while True:
        page = production.representations_page(asset_id, limit=2, cursor=cursor)
        for representation in page.items:
            print(f"{representation.kind.name} {representation.structure_kind.name}")
            for member in representation.members:
                print(f"  member {member.role or '-'} required={member.required}")
            for resource in representation.resources:
                for fingerprint in resource.fingerprints:
                    print(f"  resource {fingerprint.algorithm} v{fingerprint.version}")
                for locator in resource.locators:
                    # A sequence locator names its directory and the files there.
                    naming = locator.sequence_naming
                    files = f" {naming.filename(1001)}" if naming is not None else ""
                    print(
                        f"  locator {locator.uri}{files} ({locator.availability.name})"
                    )
            # Representation fingerprints are separate from resource ones.
            for fingerprint in representation.fingerprints:
                print(f"  representation {fingerprint.algorithm}")
            sequence = representation.image_sequence
            if sequence is not None:
                print(
                    f"  frames {sequence.start}-{sequence.end}, "
                    f"missing {list(sequence.missing_frames)}"
                )
            described.append(representation)
        cursor = page.next_cursor
        if cursor is None:
            return described


# [/representation-structure]


# [media-root-lifecycle]
def find_root(production: Production, name: str) -> MediaRootId:
    for root in production.media_roots:  # in priority order
        print(f"root {root.name}: enabled={root.enabled} priority={root.priority}")
    return next(root.id for root in production.media_roots if root.name == name)


def set_root_enabled(
    production: Production, root_id: MediaRootId, enabled: bool
) -> None:
    # A disabled root stays configured but is skipped by resolution.
    with production.read_session() as view, view.edit() as transaction:
        transaction.set_media_root_enabled(root_id, enabled)
        transaction.commit()


def remove_root(production: Production, root_id: MediaRootId) -> None:
    with production.read_session() as view, view.edit() as transaction:
        transaction.remove_media_root(root_id)
        transaction.commit()


# [/media-root-lifecycle]


# [retire-locator]
def retire_superseded_locator(
    production: Production, resource_id: ResourceId, superseded_uri: str
) -> list[LocatorMatch]:
    with production.transaction() as transaction:
        for match in production.locators_page(resource_id, limit=100).items:
            if match.locator.uri == superseded_uri:
                transaction.retire_locator(match.locator.id)
        transaction.commit()

    remaining: list[LocatorMatch] = []
    cursor = None
    while True:
        page = production.locators_page(resource_id, limit=100, cursor=cursor)
        remaining.extend(page.items)
        cursor = page.next_cursor
        if cursor is None:
            return remaining


# [/retire-locator]


# [locator-uri]
def is_recorded_locator(path: Path, recorded_uri: str) -> bool:
    # Spell the path as PostProject spells locators instead of building a URI
    # with pathlib or urllib, then compare the strings exactly.
    print(f"recorded locator {recorded_uri} is {locator_file_path(recorded_uri)}")
    return file_locator(path) == recorded_uri


# [/locator-uri]


# [content-fingerprint]
def print_file_fingerprint(path: Path) -> Fingerprint:
    # The same value import records; computing it records nothing.
    fingerprint = fingerprint_file(path)
    print(f"{fingerprint.algorithm} v{fingerprint.version}: {fingerprint.value.hex()}")
    return fingerprint


# [/content-fingerprint]


# [fingerprint-observation]
def observe_fingerprints(
    production: Production, representation: Representation, path: Path
) -> tuple[RevisionEvent, ...]:
    (resource,) = representation.resources
    # Verification only reads: it compares the file with the stored value.
    assert production.verify_resource(resource.id, path) is ContentVerification.DIFFERS
    with production.transaction() as transaction:
        # Stages the new resource fingerprint and every representation
        # fingerprint recomputed from it; commit records both in one revision.
        outcome = transaction.observe_resource_content(resource.id, path)
        assert outcome is ContentObservationOutcome.CHANGED
        transaction.commit()
    latest = production.latest_revision
    assert latest is not None

    # Observing the same content again changes nothing and records nothing.
    with production.transaction() as transaction:
        outcome = transaction.observe_resource_content(resource.id, path)
        assert outcome is ContentObservationOutcome.UNCHANGED
        transaction.commit()
    assert production.latest_revision == latest

    events = production.revision_events[latest.id]
    for event in events:
        print(f"event {event.position}: {type(event.payload).__name__}")
    return events


# [/fingerprint-observation]


# [resolution-issues]
def report_availability_issues(
    production: Production, asset_id: AssetId
) -> list[AvailabilityIssue]:
    issues: list[AvailabilityIssue] = []
    for representation in production.resolve(asset_id):
        print(f"{representation.representation_id}: {representation.availability.name}")
        for issue in representation.issues:
            # Missing frames are sorted individual frame numbers.
            print(f"  issue {issue.kind.name} frames={list(issue.frames)}")
            issues.append(issue)
        for resource in representation.resources:
            for evidence in resource.evidence:
                print(
                    f"  {resource.resource_id} {resource.state.name}: "
                    f"{evidence.kind.name} {evidence.detail or ''}"
                )
    return issues


# [/resolution-issues]


def write(path: Path, content: bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return path


def add_render_sequence(
    production: Production, asset_id: AssetId, directory: Path
) -> RepresentationId:
    with production.transaction() as transaction:
        representation_id = transaction.add_representation(
            asset_id,
            RepresentationKind.DERIVED,
            ImageSequenceSource(
                directory=directory,
                naming=SequenceNaming("shot010.", ".exr", 4),
                start=1001,
                end=1004,
                step=1,
                rate_numerator=24,
                rate_denominator=1,
                missing_frames=(1003,),
            ),
        )
        transaction.commit()
        return representation_id


# [verify-resolution]
def verify_contents(production: Production, asset_id: AssetId) -> int:
    # Content mode re-fingerprints files at known locators instead of trusting
    # their presence.
    verified = 0
    for representation in production.resolve(
        asset_id, verification=VerificationMode.CONTENT
    ):
        for resource in representation.resources:
            if resource.state is ResourceResolutionState.ONLINE_AT_KNOWN_LOCATOR:
                verified += 1
            elif resource.state is ResourceResolutionState.ERROR:
                # FINGERPRINT_MISMATCH evidence: the content was replaced.
                print(f"{resource.resource_id} content differs")
    return verified


# [/verify-resolution]


# [resolve-scope]
def find_nearby(
    production: Production,
    asset_ids: list[AssetId],
    directory: Path,
    cancel_token: CancelToken,
) -> str | None:
    # A search directory is an unnamed, machine-local place such as the project
    # folder or where the media used to be; it is never recorded. Each searched
    # directory has its own budget, and another thread may cancel the token.
    # All assets are resolved together; each directory is scanned once.
    for representation in production.resolve(
        asset_ids,
        search_directories=[directory],
        verification=VerificationMode.PRESENCE,
        max_depth=16,
        max_entries_per_directory=50_000,
        cancel_token=cancel_token,
    ):
        for resource in representation.resources:
            discovered = resource.state in (
                ResourceResolutionState.RESOLVED_EXACT,
                ResourceResolutionState.RESOLVED_PROBABLE,
            )
            # A candidate from a search directory has no media root.
            if discovered and len(resource.candidates) == 1:
                (candidate,) = resource.candidates
                if candidate.media_root is None:
                    return candidate.uri
    return None


# [/resolve-scope]


def relocate_under_root(
    production: Production,
    asset_id: AssetId,
    representation_id: RepresentationId,
    source: Path,
    root_directory: Path,
) -> None:
    """Move a file under a root directory and confirm it under root ``proxies``."""

    source.rename(root_directory / source.name)
    with production.transaction() as transaction:
        transaction.add_media_root("proxies", "Proxy volume")
        transaction.commit()
    (resolution,) = [
        item
        for item in production.resolve(asset_id, {"proxies": root_directory})
        if item.representation_id == representation_id
    ]
    (resource,) = resolution.resources
    (candidate,) = resource.candidates
    assert candidate.media_root == "proxies"
    assert any(
        evidence.kind is EvidenceKind.MEDIA_ROOT_RELATION
        for evidence in candidate.evidence
    )
    with production.transaction() as transaction:
        transaction.confirm_locator(
            resource.resource_id, candidate.uri, media_root="proxies"
        )
        transaction.commit()


# [relink-renamed-sequence]
def relink_renamed_sequence(
    production: Production, asset_id: AssetId, directory: Path
) -> SequenceNaming | None:
    for representation in production.resolve(asset_id, search_directories=[directory]):
        for resource in representation.resources:
            # A renamed sequence is found by content; the candidate carries the
            # naming its files have now.
            if len(resource.candidates) != 1:
                continue
            candidate = resource.candidates[0]
            if candidate.sequence_naming is None:
                continue
            for evidence in candidate.evidence:
                print(f"{candidate.uri}: {evidence.kind.name}")
            with production.transaction() as transaction:
                transaction.confirm_locator(
                    resource.resource_id,
                    candidate.uri,
                    media_root=candidate.media_root,
                    sequence_naming=candidate.sequence_naming,
                )
                transaction.commit()
            return candidate.sequence_naming
    return None


# [/relink-renamed-sequence]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: media.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    rushes = work / "rushes"
    media = rushes / "A001.mov"

    with Production.create(work / "media.pproj", "Media") as production:
        with production.transaction() as transaction:
            asset_id = transaction.import_media(media, display_name="Camera A")
            transaction.commit()
        original = production.representations[asset_id][0]

        proxy = write(work / "proxies" / "A001_proxy.mov", b"proxy essence\n")
        proxy_id = add_proxy(production, asset_id, proxy)

        write(work / "spanned" / "CLIP0001.MTS", b"span one\n")
        write(work / "spanned" / "CLIP0002.MTS", b"span two\n")
        clip_id = add_spanned_clip(production, asset_id, work / "spanned")

        write(work / "package" / "clip.mxf", b"package essence\n")
        write(work / "package" / "clip.xml", b"<clip/>\n")
        package_id = add_package(production, asset_id, work / "package")

        sequence_id = add_render_sequence(
            production, asset_id, work / "renders" / "shot010"
        )

        described = {
            item.id: item for item in describe_representations(production, asset_id)
        }
        assert set(described) == {
            original.id,
            proxy_id,
            clip_id,
            package_id,
            sequence_id,
        }
        assert described[proxy_id].kind is RepresentationKind.PROXY
        clip = described[clip_id]
        assert clip.structure_kind is ContentStructureKind.ORDERED_PARTS
        assert [member.role for member in clip.members] == [
            "org.postproject:essence",
            "org.postproject:span-part",
        ]
        package = described[package_id]
        assert package.structure_kind is ContentStructureKind.PACKAGE
        assert [member.required for member in package.members] == [True, False]
        assert all(resource.fingerprints for resource in package.resources)
        assert all(resource.locators for resource in package.resources)
        assert package.fingerprints
        sequence = described[sequence_id].image_sequence
        assert sequence is not None and sequence.missing_frames == (1003,)

        with production.transaction() as transaction:
            transaction.add_media_root("archive", "Archive shelf", priority=10)
            transaction.commit()
        archive_id = find_root(production, "archive")
        set_root_enabled(production, archive_id, False)
        assert [root.enabled for root in production.media_roots] == [False]
        set_root_enabled(production, archive_id, True)
        assert [root.enabled for root in production.media_roots] == [True]
        remove_root(production, archive_id)
        assert production.media_roots == ()

        media.write_bytes(b"re-encoded camera original\n")
        assert is_recorded_locator(media, original.resources[0].locators[0].uri)
        events = observe_fingerprints(production, original, media)
        assert [type(event.payload) for event in events] == [
            ResourceFingerprintObservedEvent,
            RepresentationFingerprintObservedEvent,
        ]
        observed = next(
            item
            for item in production.representations[asset_id]
            if item.id == original.id
        )
        assert observed.fingerprints != original.fingerprints
        assert observed.resources[0].fingerprints == (print_file_fingerprint(media),)

        nearby = work / "nearby"
        nearby.mkdir()
        proxy.rename(nearby / proxy.name)
        token = CancelToken()
        found = find_nearby(production, [asset_id], nearby, token)
        assert found == (nearby / proxy.name).resolve().as_uri()
        token.cancel()
        try:
            find_nearby(production, [asset_id], nearby, token)
            raise AssertionError("cancelled resolution must raise")
        except CancelledError:
            pass
        (nearby / proxy.name).rename(proxy)

        assert verify_contents(production, asset_id) > 0
        superseded_uri = proxy.resolve().as_uri()
        moved = work / "moved"
        relocate_under_root(production, asset_id, proxy_id, proxy, moved)
        proxy_resource_id = described[proxy_id].resources[0].id
        remaining = retire_superseded_locator(
            production, proxy_resource_id, superseded_uri
        )
        assert [match.locator.uri for match in remaining] == [
            (moved / proxy.name).resolve().as_uri()
        ]
        assert remaining[0].media_root == "proxies"

        # A missing optional sidecar is reported without making the package
        # unavailable; the unmapped root appears as the resource's evidence.
        (work / "package" / "clip.xml").unlink()
        issues = report_availability_issues(production, asset_id)
        assert sorted((issue.kind.name, issue.frames) for issue in issues) == [
            (AvailabilityIssueKind.MISSING_FRAMES.name, (1003,)),
            (AvailabilityIssueKind.RESOURCE_ERROR.name, ()),
        ]
        resolutions = {
            item.representation_id: item for item in production.resolve(asset_id)
        }
        assert resolutions[package_id].availability is RepresentationAvailability.ONLINE
        sidecar = resolutions[package_id].resources[1]
        assert sidecar.evidence == (
            ResolutionEvidence(EvidenceKind.MEDIA_ROOT_UNMAPPED, "proxies"),
        )

        strip_id = import_image_strip(production, work / "renders" / "shot010")
        (strip,) = production.representations[strip_id]
        assert strip.kind is RepresentationKind.ORIGINAL
        assert strip.structure_kind is ContentStructureKind.IMAGE_SEQUENCE

        # The strip's frames are graded and renamed into another directory.
        graded = work / "graded"
        graded.mkdir()
        for frame in (1001, 1002, 1004):
            (work / "renders" / "shot010" / f"shot010.{frame}.exr").rename(
                graded / f"shot010-graded_{frame}.exr"
            )
        naming = relink_renamed_sequence(production, strip_id, graded)
        assert naming == SequenceNaming("shot010-graded_", ".exr", 4)
        (relinked,) = production.representations[strip_id]
        assert {
            locator.sequence_naming.prefix
            for locator in relinked.resources[0].locators
            if locator.sequence_naming is not None
        } == {"shot010.", "shot010-graded_"}


if __name__ == "__main__":
    main()
