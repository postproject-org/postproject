"""Run every Python listing included in the PostProject integrator guides.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python guides.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
import threading
from collections.abc import Callable
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivitySpec,
    AssetId,
    AssetRef,
    Dependency,
    ExternalIdentifier,
    ImageSequenceSource,
    JobRequest,
    JobState,
    JobSucceededEvent,
    MetadataLanguageString,
    MetadataProperty,
    ObjectReference,
    OriginIdentity,
    Production,
    RepresentationAddedEvent,
    RepresentationId,
    RepresentationKind,
    RepresentationRef,
    RepresentationResolution,
    Revision,
    RevisionEvent,
    RevisionId,
    RevisionObserver,
    SequenceNaming,
    ToolIdentity,
)


# [create-production]
def create_production(path: Path, media: Path) -> tuple[Production, AssetId]:
    production = Production.create(path, "Documentary")
    with production.transaction(
        origin=OriginIdentity("com.example.editor", "0.4.0"),
        message="Import camera original",
    ) as transaction:
        asset_id = transaction.import_media(media, display_name="Camera A")
        transaction.commit()

    print(f"representations: {len(production.representations[asset_id])}")
    return production, asset_id


# [/create-production]


# [external-identifiers]
def tag_camera_serial(production: Production, asset_id: AssetId) -> None:
    identifier = ExternalIdentifier("com.example.camera.serial", "A-0007")
    with production.transaction() as transaction:
        transaction.add_external_identifier(AssetRef(asset_id), identifier)
        transaction.commit()

    attached = production.external_identifiers[AssetRef(asset_id)]
    matches = production.objects_by_external_identifier[
        identifier.scheme, identifier.value
    ]
    assert attached == (identifier,)
    assert matches == (AssetRef(asset_id),)


# [/external-identifiers]


# [metadata]
def add_title(production: Production, asset_id: AssetId) -> None:
    title = MetadataProperty(
        "https://iptc.org/std/videometadatahub/recommendation/iptc-vmhub-1.7-schema.json",
        "title",
    )
    with production.transaction() as transaction:
        transaction.add_metadata(
            AssetRef(asset_id), title, MetadataLanguageString("Interview", "en-US")
        )
        transaction.commit()

    for assertion in production.metadata[AssetRef(asset_id)]:
        print(f"{assertion.property.property}: {assertion.value}")
    assert len(production.metadata_by_property[title]) == 1


# [/metadata]


# [media-root]
def add_rushes_root(production: Production) -> None:
    with production.transaction() as transaction:
        transaction.add_media_root("rushes", "Camera originals")
        transaction.commit()


# [/media-root]


# [resolve-asset]
def resolve_asset(
    production: Production, asset_id: AssetId, rushes_directory: Path
) -> tuple[RepresentationResolution, ...]:
    resolutions = production.resolve(asset_id, {"rushes": rushes_directory})
    for representation in resolutions:
        print(f"availability: {representation.availability.name}")
        for resource in representation.resources:
            for candidate in resource.candidates:
                print(
                    f"candidate: {candidate.uri} "
                    f"({candidate.confidence_basis_points}/10000)"
                )
    return resolutions


# [/resolve-asset]


# [confirm-locator]
def confirm_unique_candidates(
    production: Production, resolutions: tuple[RepresentationResolution, ...]
) -> None:
    with production.transaction() as transaction:
        for representation in resolutions:
            for resource in representation.resources:
                # Several candidates need a person to choose; never pick one here.
                if len(resource.candidates) != 1:
                    continue
                candidate = resource.candidates[0]
                # Record the logical root the candidate was found under and,
                # for an image sequence, the naming of its files there.
                transaction.confirm_locator(
                    resource.resource_id,
                    candidate.uri,
                    media_root=candidate.media_root,
                    sequence_naming=candidate.sequence_naming,
                )
        transaction.commit()


# [/confirm-locator]


# [image-sequence]
def add_render_sequence(
    production: Production, asset_id: AssetId, directory: Path
) -> RepresentationId:
    with production.transaction() as transaction:
        sequence_id = transaction.add_representation(
            asset_id,
            RepresentationKind.DERIVED,
            ImageSequenceSource(
                directory=directory,
                naming=SequenceNaming("shot010.", ".exr", 4),
                start=1001,
                end=1004,
                step=1,
                rate_numerator=24000,
                rate_denominator=1001,
                missing_frames=(1003,),
            ),
        )
        transaction.commit()

    stored = next(
        item for item in production.representations[asset_id] if item.id == sequence_id
    )
    sequence = stored.image_sequence
    assert sequence is not None
    print(
        f"frames {sequence.start}-{sequence.end} at "
        f"{sequence.rate_numerator}/{sequence.rate_denominator}, "
        f"{len(sequence.missing_frames)} known missing"
    )
    return sequence_id


# [/image-sequence]


# [provenance]
def record_render(
    production: Production,
    source_id: RepresentationId,
    render_id: RepresentationId,
) -> None:
    with production.transaction() as transaction:
        activity_id = transaction.create_activity(
            ActivitySpec(
                "org.postproject:render",
                inputs=(ActivityEdge(source_id, "org.postproject:primary"),),
                outputs=(ActivityEdge(render_id),),
                tool=ToolIdentity(
                    "Example Renderer", "2.1", "https://example.com/renderer"
                ),
            )
        )
        transaction.commit()

    (producer,) = production.activities_producing[render_id]
    assert producer.id == activity_id
    assert production.activities_consuming[source_id] == (producer,)
    assert production.provenance_ancestors[render_id] == (source_id,)
    assert production.provenance_descendants[source_id] == (render_id,)


# [/provenance]


# [artifact-knowledge]
def inspect_artifact(production: Production, artifact_id: RepresentationId) -> None:
    evaluation = production.evaluate_artifact(
        artifact_id, max_depth=64, max_representations=1000
    )
    print(f"artifact state: {evaluation.state.name}")
    for reason in evaluation.reasons:
        print(f"reason: {reason.kind.name}")

    reproducibility = production.artifact_reproducibility(artifact_id)
    print(
        f"reproducible: {reproducibility.reproducible}, "
        f"missing conditions: {len(reproducibility.issues)}"
    )


# [/artifact-knowledge]


# [dependency-queries]
def record_and_query_dependencies(
    production: Production,
    source_id: RepresentationId,
    target_asset_id: AssetId,
    resolved_id: RepresentationId,
) -> None:
    dependency = Dependency(
        kind="org.example:character-reference",
        target=AssetRef(target_asset_id),
        authored_reference="characters/lead.usd",
        resolved_representation_id=resolved_id,
    )
    with production.read_session() as view, view.edit() as transaction:
        transaction.record_dependency_set(source_id, (dependency,))
        transaction.commit()

    dependencies = production.dependencies(
        source_id, max_depth=4, max_representations=1000, limit=100
    )
    for match in dependencies.items:
        print(f"dependency {match.target} at depth {match.depth}")
    assert not dependencies.traversal_truncated

    dependents = production.dependents(
        AssetRef(target_asset_id), max_depth=4, max_representations=1000, limit=100
    )
    assert dependents.items[0].target == RepresentationRef(source_id)


# [/dependency-queries]


# [job-query-pages]
def request_and_page_jobs(
    production: Production,
    input_id: RepresentationId,
    output_asset_id: AssetId,
) -> None:
    request = JobRequest(
        "org.example:generate-proxy",
        (input_id,),
        output_asset_id,
        RepresentationKind.PROXY,
    )
    with production.transaction() as transaction:
        job_id = transaction.request_job(request)
        transaction.request_job(request)
        transaction.commit()
    assert production.job(job_id).state is JobState.REQUESTED

    cursor = None
    count = 0
    while True:
        page = production.jobs(
            limit=1,
            cursor=cursor,
            state=JobState.REQUESTED,
            kind="org.example:generate-proxy",
        )
        count += len(page.items)
        cursor = page.next_cursor
        if cursor is None:
            break
    assert count == 2


# [/job-query-pages]


# [media-structure-pages]
def print_recorded_locators(production: Production) -> None:
    cursor = None
    while True:
        assets = production.assets_page(limit=100, cursor=cursor)
        for asset in assets.items:
            # Follow each nested next_cursor the same way in large productions.
            for representation in production.representations_page(
                asset.id, limit=100
            ).items:
                for resource_id in production.resources_page(
                    representation.id, limit=100
                ).items:
                    for match in production.locators_page(resource_id, limit=100).items:
                        print(f"{match.locator.uri} (root: {match.media_root or '-'})")
        cursor = assets.next_cursor
        if cursor is None:
            break


# [/media-structure-pages]


# [knowledge-only-media]
def list_media_knowledge(production: Production) -> tuple[RepresentationId, ...]:
    # Both queries read recorded knowledge; neither touches the filesystem.
    unresolved = production.unresolved_media(limit=100)
    for representation_id in unresolved.items:
        print(f"no recorded locator: {representation_id}")

    under_rushes = production.representations_under_media_root("rushes", limit=100)
    return tuple(representation.id for representation in under_rushes.items)


# [/knowledge-only-media]


# [point-reads]
def read_known_objects(
    production: Production, asset_id: AssetId, representation_id: RepresentationId
) -> None:
    # A host reference names one object; read it without scanning the production.
    asset = production.asset(asset_id)
    representation = production.representation(representation_id)
    print(f"{asset.display_name}: {representation.kind.value}")
    resource_id = representation.resources[0].id
    users = production.representations_using_resource(resource_id, limit=100)
    assert representation_id in {item.id for item in users.items}


# [/point-reads]


# [metadata-query-pages]
def find_interview_titles(production: Production) -> tuple[ObjectReference, ...]:
    title = MetadataProperty(
        "https://iptc.org/std/videometadatahub/recommendation/iptc-vmhub-1.7-schema.json",
        "title",
    )
    page = production.query_metadata(
        title, limit=100, value=MetadataLanguageString("Interview", "en-US")
    )
    for assertion in page.items:
        print(f"{assertion.target}: {assertion.value}")
    return tuple(assertion.target for assertion in page.items)


# [/metadata-query-pages]


# [provenance-query-pages]
def query_render_lineage(
    production: Production,
    source_id: RepresentationId,
    render_id: RepresentationId,
) -> None:
    producing = production.activities_producing_page(render_id, limit=100)
    consuming = production.activities_consuming_page(source_id, limit=100)
    assert producing.items == consuming.items

    by_kind = production.outputs_by_activity_kind("org.postproject:render", limit=100)
    by_tool = production.outputs_by_tool(
        ToolIdentity("Example Renderer", "2.1", "https://example.com/renderer"),
        limit=100,
    )
    assert by_kind.items == by_tool.items == (render_id,)

    ancestors = production.provenance_ancestors_page(
        render_id, max_depth=8, max_representations=1000, limit=100
    )
    for match in ancestors.items:
        print(f"ancestor {match.representation_id} at depth {match.depth}")
    assert not ancestors.traversal_truncated

    descendants = production.provenance_descendants_page(
        source_id, max_depth=8, max_representations=1000, limit=100
    )
    assert descendants.items[0].representation_id == render_id


# [/provenance-query-pages]


# [stale-artifact-pages]
def stale_descendants(
    production: Production, source_id: RepresentationId
) -> list[RepresentationId]:
    stale: list[RepresentationId] = []
    cursor = None
    while True:
        page = production.stale_artifacts(
            max_depth=64,
            max_representations=1000,
            limit=100,
            cursor=cursor,
            source=source_id,
        )
        # A page bounds the candidates examined, so it may hold fewer stale
        # results, or none, and still carry a continuation.
        stale.extend(page.items)
        cursor = page.next_cursor
        if cursor is None:
            return stale


# [/stale-artifact-pages]


# [changed-objects]
def objects_changed_after(
    production: Production, sequence: int
) -> list[ObjectReference]:
    changed: list[ObjectReference] = []
    cursor = None
    while True:
        page = production.objects_changed_since(sequence, limit=100, cursor=cursor)
        changed.extend(page.items)
        cursor = page.next_cursor
        if cursor is None:
            return changed


# [/changed-objects]


def handle_event(event: RevisionEvent) -> None:
    print(f"event {event.position}: {type(event.payload).__name__}")


# [revision-feed]
def process_changes(production: Production, cursor: int) -> int:
    limit = 100
    while True:
        page = production.changes_since(cursor, limit)
        for revision in page:
            for event in production.revision_events[revision.id]:
                # Dispatch with isinstance on event.payload.
                handle_event(event)
            # Persist the cursor only after the whole revision is processed.
            cursor = revision.sequence
        if len(page) < limit:
            return cursor


# [/revision-feed]


# [revision-filter]
def new_media_revisions(
    production: Production, cursor: int
) -> tuple[list[RevisionId], int]:
    page = production.changes_since_filtered(
        cursor, [RepresentationAddedEvent, JobSucceededEvent]
    )
    # Continue from the through sequence, which skips unrelated revisions.
    return [revision.id for revision in page.revisions], page.through_sequence


# [/revision-filter]


# [revision-wait]
def wait_for_changes(production: Production, cursor: int) -> tuple[Revision, ...]:
    with production.revision_waiter() as waiter:
        # Another thread may call waiter.cancel() to stop the wait.
        return waiter.wait(cursor, timeout=5).revisions  # empty unless revisions


def watch_new_media(
    production: Production,
    cursor: int,
    on_revision: Callable[[Revision, tuple[RevisionEvent, ...]], object],
) -> RevisionObserver:
    # on_revision runs on the observer's own thread; call stop() before closing.
    return RevisionObserver(
        production,
        on_revision,
        after_sequence=cursor,
        kinds=[RepresentationAddedEvent],
    )


# [/revision-wait]


# [host-binding]
def bind_representation(
    production: Production, representation_id: RepresentationId
) -> str:
    stored = production.host_bindings[RepresentationRef(representation_id)]

    binding = production.host_bindings.parse(stored)
    assert binding.production_id == production.id
    assert binding.object == RepresentationRef(representation_id)
    return stored


# [/host-binding]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: guides.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    media = work / "rushes" / "A001.mov"
    moved = work / "moved"

    production, asset_id = create_production(work / "production.pproj", media)
    with production:
        original_id = production.representations[asset_id][0].id
        tag_camera_serial(production, asset_id)
        add_title(production, asset_id)

        add_rushes_root(production)
        media.rename(moved / "A001.mov")
        resolutions = resolve_asset(production, asset_id, moved)
        assert len(resolutions[0].resources[0].candidates) == 1
        confirm_unique_candidates(production, resolutions)

        before_render = production.latest_revision
        assert before_render is not None
        sequence_id = add_render_sequence(
            production, asset_id, work / "renders" / "shot010"
        )
        record_render(production, original_id, sequence_id)
        inspect_artifact(production, sequence_id)
        record_and_query_dependencies(production, sequence_id, asset_id, original_id)
        request_and_page_jobs(production, original_id, asset_id)

        print_recorded_locators(production)
        assert list_media_knowledge(production) == (original_id,)
        read_known_objects(production, asset_id, original_id)
        assert find_interview_titles(production) == (AssetRef(asset_id),)
        query_render_lineage(production, original_id, sequence_id)
        assert stale_descendants(production, original_id) == []
        assert RepresentationRef(sequence_id) in objects_changed_after(
            production, before_render.sequence
        )

        cursor = process_changes(production, 0)
        latest = production.latest_revision
        assert latest is not None and cursor == latest.sequence
        media_revisions, through = new_media_revisions(production, 0)
        assert media_revisions and through == cursor
        assert wait_for_changes(production, 0)
        delivered = threading.Event()
        observer = watch_new_media(production, 0, lambda *_: delivered.set())
        assert delivered.wait(timeout=60)
        observer.stop()
        assert observer.error is None
        print(f"binding: {bind_representation(production, sequence_id)}")


if __name__ == "__main__":
    main()
