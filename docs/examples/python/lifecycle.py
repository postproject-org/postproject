"""Run the Python production and transaction lifecycle listings.

Each ``[name]`` ... ``[/name]`` region is included verbatim by the
documentation build, so keep regions self-contained and readable. Usage::

    POSTPROJECT_LIBRARY=/path/to/libpostproject.so python lifecycle.py WORK_DIRECTORY

The work directory is prepared by ``prepare-workdir.cmake``.
"""

from __future__ import annotations

import sys
from pathlib import Path
from uuid import UUID

from postproject import (
    Asset,
    AssetId,
    AssetRef,
    ExternalIdentifier,
    NotFoundError,
    OriginIdentity,
    Production,
    ProductionId,
    Revision,
    RevisionContext,
    parse_id,
)


# [asset-identity]
def asset_identity(saved_id: str) -> None:
    asset = parse_id(saved_id, AssetId)
    assert isinstance(asset, UUID)
    assert AssetId(asset) is asset
    assert len({asset, asset}) == 1
    print("asset identity:", asset)


# [/asset-identity]
# [open-production]
def open_production(
    path: Path, asset_id: AssetId
) -> tuple[ProductionId, list[Asset], Revision | None]:
    production = Production.open(path)
    try:
        production_id = production.id
        print(f"production: {production_id}")
        if asset_id in production.assets:
            print(f"asset {asset_id} exists")

        assets = list(production.assets)
        for asset in assets:
            print(f"asset {asset.id}: {asset.display_name or '-'}")

        latest = production.latest_revision  # None for an empty production
        if latest is not None:
            print(f"latest revision {latest.sequence}: {latest.message or '-'}")
        return production_id, assets, latest
    finally:
        production.close()


# [/open-production]


# [transaction-lifecycle]
def tag_then_discard(production: Production, asset_id: AssetId) -> None:
    transaction = production.transaction()
    try:
        transaction.set_revision_context(
            RevisionContext(
                OriginIdentity("com.example.editor", "0.4.0"), "Tag camera serial"
            )
        )
        transaction.add_external_identifier(
            AssetRef(asset_id),
            ExternalIdentifier("com.example.camera.serial", "A-0007"),
        )
        transaction.commit()
    finally:
        transaction.close()  # releases the handle; harmless after commit

    before = production.latest_revision
    transaction = production.transaction()
    try:
        transaction.add_external_identifier(
            AssetRef(asset_id), ExternalIdentifier("com.example.tape", "T-0012")
        )
        transaction.rollback()
    finally:
        # Closing a transaction that was neither committed nor rolled back
        # rolls it back as well.
        transaction.close()

    # A rollback creates no revision and leaves no staged change behind.
    assert production.latest_revision == before
    assert len(production.external_identifiers[AssetRef(asset_id)]) == 1


# [/transaction-lifecycle]


# [error-handling]
def open_missing(path: Path) -> NotFoundError | None:
    try:
        Production.open(path)
    except NotFoundError as error:
        # Every PostProjectError carries the stable C ABI code and a message.
        print(f"not found (code {error.code}): {error}")
        return error
    return None


# [/error-handling]


def create_production(path: Path, media: Path) -> tuple[ProductionId, AssetId]:
    with Production.create(path, "Lifecycle") as production:
        with production.transaction(message="Import camera original") as transaction:
            asset_id = transaction.import_media(media, display_name="Camera A")
            transaction.commit()
        return production.id, asset_id


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: lifecycle.py WORK_DIRECTORY")
    work = Path(sys.argv[1])
    asset_identity("00000000-0000-0000-0000-000000000001")
    path = work / "lifecycle.pproj"

    created_id, asset_id = create_production(path, work / "rushes" / "A001.mov")
    production_id, assets, latest = open_production(path, asset_id)
    assert production_id == created_id
    assert [asset.id for asset in assets] == [asset_id]
    assert assets[0].display_name == "Camera A"
    assert latest is not None and latest.sequence == 1
    assert latest.message == "Import camera original"

    with Production.open(path) as production:
        tag_then_discard(production, asset_id)
        revision = production.latest_revision
        assert revision is not None and revision.sequence == 2
        assert revision.message == "Tag camera serial"
        assert revision.origin == OriginIdentity("com.example.editor", "0.4.0")
        assert production.external_identifiers[AssetRef(asset_id)] == (
            ExternalIdentifier("com.example.camera.serial", "A-0007"),
        )

    error = open_missing(work / "missing.pproj")
    assert error is not None and error.code == 2
    assert not (work / "missing.pproj").exists()


if __name__ == "__main__":
    main()
