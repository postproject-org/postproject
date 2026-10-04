"""Run the Python provenance, staleness, and dependency listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python provenance.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

from postproject import (
    Activity,
    ActivityEdge,
    ActivityId,
    ActivitySpec,
    AgentIdentity,
    ArtifactEvaluation,
    ArtifactKnowledgeState,
    ArtifactReasonKind,
    ArtifactReproducibility,
    ArtifactReproducibilityIssueKind,
    AssetId,
    AssetRef,
    Dependency,
    DependencyMatch,
    DependencySet,
    DependencySetStatus,
    ExternalIdentifier,
    Fingerprint,
    Production,
    Representation,
    RepresentationId,
    RepresentationKind,
    RepresentationRef,
    ToolIdentity,
)


# [activity-snapshots]
def record_proxy_generation(
    production: Production,
    original_id: RepresentationId,
    proxy_id: RepresentationId,
) -> ActivityId:
    with production.transaction() as transaction:
        activity_id = transaction.create_activity(
            ActivitySpec(
                "org.postproject:transcode",
                inputs=(ActivityEdge(original_id, "org.postproject:primary"),),
                outputs=(ActivityEdge(proxy_id, "org.postproject:proxy"),),
                tool=ToolIdentity("Example Transcoder", "3.2"),
                agent=AgentIdentity(
                    "Render node 4", ExternalIdentifier("com.example.host", "node-4")
                ),
            )
        )
    return activity_id


def print_fingerprint_snapshots(edge: ActivityEdge) -> None:
    # Commit captured the fingerprints each edge representation had then.
    if edge.snapshot is None:
        return
    for fingerprint in edge.snapshot.fingerprints:
        print(
            f"  {edge.representation_id} {fingerprint.algorithm}: "
            f"{fingerprint.value.hex()[:16]}..."
        )


def print_activities(production: Production) -> tuple[Activity, ...]:
    activities = production.activities
    for activity in activities:
        tool = activity.tool.name if activity.tool else "-"
        agent = activity.agent.name if activity.agent else "-"
        print(f"{activity.kind} by {tool} on {agent}")
        for edge in activity.inputs + activity.outputs:
            print(f" {edge.role or '-'}")
            print_fingerprint_snapshots(edge)
    return activities


def print_lineage(production: Production, original_id: RepresentationId) -> None:
    for activity in production.activities_consuming[original_id]:
        print(f"consumed by {activity.id}")
    for descendant_id in production.provenance_descendants[original_id]:
        print(f"descendant {descendant_id}")

    cursor = None
    while True:
        page = production.provenance_descendants_page(
            original_id, max_depth=8, max_representations=1000, limit=100, cursor=cursor
        )
        for match in page.items:
            print(f"descendant {match.representation_id} at depth {match.depth}")
        cursor = page.next_cursor
        if cursor is None:
            break


# [/activity-snapshots]


# [stale-after-change]
def record_fingerprints(
    production: Production, representation: Representation, path: Path
) -> None:
    # Observations in the caller's own fingerprint domain.
    digest = hashlib.sha256(path.read_bytes()).digest()
    (resource,) = representation.resources
    with production.transaction() as transaction:
        transaction.record_resource_fingerprint(
            resource.id, Fingerprint("example-sha256", 1, digest)
        )
        transaction.record_representation_fingerprint(
            representation.id,
            Fingerprint(
                "example-sha256-representation", 1, hashlib.sha256(digest).digest()
            ),
        )


def explain_proxy(
    production: Production, proxy_id: RepresentationId
) -> tuple[ArtifactEvaluation, ArtifactReproducibility]:
    evaluation = production.evaluate_artifact(
        proxy_id, max_depth=64, max_representations=1000
    )
    print(f"proxy is {evaluation.state.name}")
    for reason in evaluation.reasons:
        print(f"  {reason.kind.name} on {reason.representation_id}")
        if reason.kind is ArtifactReasonKind.FINGERPRINT_CHANGED:
            assert reason.snapshot_value is not None
            assert reason.current_value is not None
            print(
                f"  {reason.fingerprint_algorithm}: "
                f"{reason.snapshot_value.hex()[:16]} -> {reason.current_value.hex()[:16]}"
            )

    # What is still missing to regenerate the proxy exactly.
    reproducibility = production.artifact_reproducibility(proxy_id)
    for issue in reproducibility.issues:
        print(f"  not reproducible: {issue.kind.name}")
    return evaluation, reproducibility


# [/stale-after-change]


# [dependency-set]
def record_edit_dependencies(
    production: Production,
    edit_id: RepresentationId,
    camera_asset_id: AssetId,
    original_id: RepresentationId,
    proxy_id: RepresentationId,
) -> DependencySet | None:
    # Always record the complete set: it replaces the previous observation.
    dependencies = (
        Dependency(
            "org.example:clip-reference",
            AssetRef(camera_asset_id),
            "rushes/A001.mov",
            resolved_representation_id=original_id,
        ),
        Dependency(
            "org.example:offline-proxy",
            RepresentationRef(proxy_id),
            "proxies/A001_proxy.mov",
            required=False,
        ),
    )
    with production.transaction() as transaction:
        transaction.record_dependency_set(edit_id, dependencies)

    recorded = production.dependency_set(edit_id)  # None: never extracted
    if recorded is not None:
        print(f"{len(recorded.dependencies)} dependencies, {recorded.status.name}")
    return recorded


def all_dependencies(
    production: Production, edit_id: RepresentationId
) -> list[DependencyMatch]:
    matches: list[DependencyMatch] = []
    cursor = None
    while True:
        page = production.dependencies(
            edit_id, max_depth=1, max_representations=1000, limit=1, cursor=cursor
        )
        matches.extend(page.items)
        cursor = page.next_cursor
        if cursor is None:
            return matches


# [/dependency-set]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: provenance.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    media = work / "rushes" / "A001.mov"
    proxy = work / "proxies" / "A001_proxy.mov"
    proxy.parent.mkdir()
    proxy.write_bytes(b"proxy essence\n")
    edit = work / "edit" / "cut.otio"
    edit.parent.mkdir()
    edit.write_text("{}\n", encoding="utf-8")

    with Production.create(work / "provenance.pproj", "Provenance") as production:
        with production.transaction() as transaction:
            asset_id = transaction.import_media(media)
            edit_asset_id = transaction.import_media(edit)
        original = production.representations[asset_id][0]
        edit_representation = production.representations[edit_asset_id][0]
        with production.transaction() as transaction:
            proxy_id = transaction.add_representation(
                asset_id, RepresentationKind.PROXY, proxy
            )
        # Establish the caller's fingerprint domain before the activity, so
        # that the activity snapshots capture it.
        record_fingerprints(production, original, media)

        activity_id = record_proxy_generation(production, original.id, proxy_id)
        (activity,) = print_activities(production)
        assert activity.id == activity_id
        assert activity.tool == ToolIdentity("Example Transcoder", "3.2")
        assert activity.agent is not None
        assert activity.agent.identifier == ExternalIdentifier(
            "com.example.host", "node-4"
        )
        input_snapshot = activity.inputs[0].snapshot
        assert input_snapshot is not None
        assert {item.algorithm for item in input_snapshot.fingerprints} == {
            "example-sha256-representation",
            "pp-blake3-representation",
        }
        assert activity.outputs[0].snapshot is not None
        print_lineage(production, original.id)
        assert production.activities_consuming[original.id] == (activity,)
        assert production.provenance_descendants[original.id] == (proxy_id,)

        evaluation, _ = explain_proxy(production, proxy_id)
        assert evaluation.state is ArtifactKnowledgeState.CURRENT

        media.write_bytes(b"re-graded camera original\n")
        record_fingerprints(production, original, media)
        evaluation, reproducibility = explain_proxy(production, proxy_id)
        assert evaluation.state is ArtifactKnowledgeState.STALE
        assert [reason.kind for reason in evaluation.reasons] == [
            ArtifactReasonKind.FINGERPRINT_CHANGED
        ]
        assert evaluation.reasons[0].fingerprint_algorithm == (
            "example-sha256-representation"
        )
        assert not reproducibility.reproducible
        assert [issue.kind for issue in reproducibility.issues] == [
            ArtifactReproducibilityIssueKind.PARAMETERS_MISSING
        ]

        recorded = record_edit_dependencies(
            production, edit_representation.id, asset_id, original.id, proxy_id
        )
        assert recorded is not None
        assert recorded.status is DependencySetStatus.CURRENT
        assert len(recorded.dependencies) == 2
        assert [
            match.target
            for match in all_dependencies(production, edit_representation.id)
        ] == [
            AssetRef(asset_id),
            RepresentationRef(proxy_id),
        ]

        # A newer content observation of the edit invalidates its dependency set.
        edit.write_text('{"tracks": []}\n', encoding="utf-8")
        record_fingerprints(production, edit_representation, edit)
        updated = production.dependency_set(edit_representation.id)
        assert updated is not None
        assert updated.status is DependencySetStatus.NEEDS_EXTRACTION
        assert production.dependency_set(proxy_id) is None


if __name__ == "__main__":
    main()
