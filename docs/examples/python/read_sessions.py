"""Pinned views and explicit-commit edits through the installed binding."""

import sys
from pathlib import Path

from postproject import (
    AssetRef,
    ContentVerification,
    DecisionBase,
    ExternalIdentifier,
    LocatorIdentity,
    MetadataProperty,
    MetadataString,
    Production,
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
        with production.read_session() as view:
            copied = view.asset(asset)
            page = view.representations_page(asset, limit=10)
            assert len(page.items) == 1
            representation = view.representation(page.items[0].id)
            title = MetadataProperty("https://example.com/editorial", "title")
            metadata = view.metadata(AssetRef(asset))
            assert metadata[0].value == MetadataString("Camera")
            assert view.metadata_by_property(title) == metadata
            assert view.query_metadata(title, limit=10).items == metadata
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
        production.close()
        view.close()  # close is idempotent


# [/coherent-reads]

if __name__ == "__main__":
    directory = Path(sys.argv[1])
    exercise(directory / "views.pproj", directory / "rushes/A001.mov")
