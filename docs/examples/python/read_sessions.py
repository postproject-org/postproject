"""Pinned views and explicit-commit edits through the installed binding."""

import sys
from pathlib import Path

from postproject import (
    ArtifactKnowledgeState,
    AssetImportedEvent,
    AssetRef,
    ContentVerification,
    DecisionBase,
    ExternalIdentifier,
    JobRequest,
    JobState,
    LocatorIdentity,
    MetadataProperty,
    MetadataString,
    NotFoundError,
    Production,
    RepresentationKind,
    ToolIdentity,
    file_locator,
    fingerprint_file,
)


# [coherent-reads]
def exercise(path: Path, media: Path) -> None:
    with Production.create(path) as production:
        with production.read_session() as empty:
            with empty.edit() as edit:
                asset = edit.import_media(media)
                edit.add_external_identifier(
                    AssetRef(asset),
                    ExternalIdentifier("https://example.com/id", "camera"),
                )
                edit.add_metadata(
                    AssetRef(asset),
                    MetadataProperty("https://example.com/editorial", "title"),
                    MetadataString("Camera"),
                )
                receipt = edit.commit()
            assert receipt.revision is not None and receipt.revision.sequence == 1
            assert empty.assets_page(limit=10).items == ()
            assert empty.latest_revision is None
            assert empty.changes_since(0, 10) == ()
        with production.read_session() as view:
            latest = view.latest_revision
            assert latest is not None and latest.id == receipt.revision.id
            assert view.changes_since(0, 10) == (latest,)
            filtered = view.changes_since_filtered(0, (AssetImportedEvent,), 10)
            assert filtered.revisions == (latest,)
            assert filtered.through_sequence == latest.sequence
            assert view.unresolved_media(limit=10).items == ()
            assert view.objects_changed_since(0, limit=10).items
            try:
                view.representations_under_media_root("missing", limit=10)
            except NotFoundError:
                pass
            else:
                raise AssertionError("unknown logical root should be rejected")
            copied = view.asset(asset)
            page = view.representations_page(asset, limit=10)
            assert len(page.items) == 1
            representation = view.representation(page.items[0].id)
            title = MetadataProperty("https://example.com/editorial", "title")
            metadata = view.metadata(AssetRef(asset))
            assert metadata[0].value == MetadataString("Camera")
            assert view.metadata_by_property(title) == metadata
            assert view.query_metadata(title, limit=10).items == metadata
            assert (
                view.evaluate_artifact(representation.id).state
                == ArtifactKnowledgeState.INDETERMINATE
            )
            assert not view.artifact_reproducibility(representation.id).reproducible
            assert view.dependency_set(representation.id) is None
            assert (
                view.dependencies(
                    representation.id, max_depth=64, max_representations=1000, limit=10
                ).items
                == ()
            )
            assert (
                view.dependents(
                    AssetRef(asset), max_depth=64, max_representations=1000, limit=10
                ).items
                == ()
            )
            assert (
                view.provenance_ancestors_page(
                    representation.id, max_depth=64, max_representations=1000, limit=10
                ).items
                == ()
            )
            assert (
                view.provenance_descendants_page(
                    representation.id, max_depth=64, max_representations=1000, limit=10
                ).items
                == ()
            )
            assert (
                view.stale_artifacts(
                    max_depth=64, max_representations=1000, limit=10
                ).items
                == ()
            )
            assert view.outputs_by_activity_kind("example:render", limit=10).items == ()
            assert view.outputs_by_tool(ToolIdentity("Example"), limit=10).items == ()
            assert (
                view.activities_producing_page(representation.id, limit=10).items == ()
            )
            assert (
                view.activities_consuming_page(representation.id, limit=10).items == ()
            )
            resources = view.resources_page(representation.id, limit=10)
            assert len(resources.items) == 1
            resource = resources.items[0]
            assert view.verify_resource(resource, media) == ContentVerification.MATCHES
            assert len(view.resolve(asset)) == 1
            assert len(view.locators_page(resource, limit=10).items) == 1
            assert view.representations_using_resource(resource, limit=10).items == (
                representation,
            )
            assert view.media_roots == ()
            assert view.external_identifiers(AssetRef(asset))[0].value == "camera"
            assert view.find_by_external_identifier(
                "https://example.com/id", "camera", None
            ) == (AssetRef(asset),)
            assert (
                view.find_known_media_by_locator(
                    LocatorIdentity(file_locator(media)), limit=10
                )
                .items[0]
                .asset_id
                == asset
            )
            assert (
                view.find_known_media_by_fingerprint(fingerprint_file(media), limit=10)
                .items[0]
                .asset_id
                == asset
            )
            base = view.decision_base
            assert DecisionBase.from_token(base.to_token()) == base
        with production.edit(base) as later:
            assert later.commit().revision is None
        # Exiting an uncommitted edit rolls back, even without an exception.
        with production.edit(base) as discarded:
            discarded.add_media_root("discarded")
        assert production.media_roots == ()
        assert copied.id == asset and representation.asset_id == asset
        with production.read_session() as before_job:
            with before_job.edit() as request:
                job = request.request_job(
                    JobRequest(
                        "example:proxy",
                        (representation.id,),
                        asset,
                        RepresentationKind.PROXY,
                    )
                )
                request.commit()
            assert before_job.jobs(limit=10).items == ()
        with production.read_session() as after_job:
            assert after_job.job(job).id == job
            assert (
                len(
                    after_job.jobs(
                        limit=10, state=JobState.REQUESTED, kind="example:proxy"
                    ).items
                )
                == 1
            )
        production.close()
        view.close()  # close is idempotent


# [/coherent-reads]

if __name__ == "__main__":
    directory = Path(sys.argv[1])
    exercise(directory / "views.pproj", directory / "rushes/A001.mov")
