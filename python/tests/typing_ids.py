"""Positive nominal-ID fixture; checked with the package's regular typing gate."""

from typing import assert_type
from uuid import UUID

from postproject import (
    AssetId,
    AssetRef,
    MediaRootId,
    Production,
    RepresentationId,
    Transaction,
    parse_id,
)


def correct_kinds(
    production: Production,
    transaction: Transaction,
    asset: AssetId,
    representation: RepresentationId,
    root: MediaRootId,
) -> None:
    assert_type(parse_id(str(UUID(int=1)), AssetId), AssetId)
    assert_type(production.asset(asset).id, AssetId)
    assert_type(production.representation(representation).id, RepresentationId)
    assert_type(AssetRef(asset).id, AssetId)
    assert_type(transaction.add_media_root("rushes"), MediaRootId)
    transaction.set_media_root_enabled(root, False)
