"""Nominal identity hints over ordinary, unchanged uuid.UUID values."""

from collections.abc import Callable
from typing import NewType, TypeVar
from uuid import UUID

ProductionId = NewType("ProductionId", UUID)
AssetId = NewType("AssetId", UUID)
RepresentationId = NewType("RepresentationId", UUID)
ResourceId = NewType("ResourceId", UUID)
LocatorId = NewType("LocatorId", UUID)
MediaRootId = NewType("MediaRootId", UUID)
ActivityId = NewType("ActivityId", UUID)
JobId = NewType("JobId", UUID)
JobClaimId = NewType("JobClaimId", UUID)
RevisionId = NewType("RevisionId", UUID)
TransactionId = NewType("TransactionId", UUID)

Identity = TypeVar("Identity", bound=UUID)


def parse_id(text: str, kind: Callable[[UUID], Identity]) -> Identity:
    """Parse UUID text and annotate its kind; the store checks existence/scope."""
    return kind(UUID(text))
