"""Positive nominal-ID fixture; checked with the package's regular typing gate."""

from typing import assert_type
from uuid import UUID

from postproject import AssetId, AssetRef, Production, RepresentationId, parse_id


def correct_kinds(
    production: Production, asset: AssetId, representation: RepresentationId
) -> None:
    assert_type(parse_id(str(UUID(int=1)), AssetId), AssetId)
    assert_type(production.asset(asset).id, AssetId)
    assert_type(production.representation(representation).id, RepresentationId)
    assert_type(AssetRef(asset).id, AssetId)
