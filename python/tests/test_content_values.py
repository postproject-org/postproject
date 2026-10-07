"""Representation facts carry one checked, independently owned content shape."""

import unittest
from dataclasses import replace
from fractions import Fraction
from uuid import UUID

from postproject import (
    AssetId,
    ContentStructureKind,
    ImageSequenceContent,
    ImageSequenceDescriptor,
    OrderedPartsContent,
    PackageContent,
    Representation,
    RepresentationId,
    RepresentationKind,
    RepresentationMember,
    ResourceId,
    SingleResourceContent,
)


class ContentValueTests(unittest.TestCase):
    def test_content_owns_members_and_derives_inspection_fields(self) -> None:
        first, second = ResourceId(UUID(int=1)), ResourceId(UUID(int=2))
        members = [
            RepresentationMember(first, "example:essence", True),
            RepresentationMember(second, "example:sidecar", False),
        ]
        content = PackageContent(members)
        members.clear()
        value = Representation(
            RepresentationId(UUID(int=3)),
            AssetId(UUID(int=4)),
            RepresentationKind.ORIGINAL,
            content,
            (),
            (),
        )
        self.assertEqual(value.structure_kind, ContentStructureKind.PACKAGE)
        self.assertEqual(len(value.members), 2)
        self.assertIsNone(value.image_sequence)
        single = replace(value, content=SingleResourceContent(first))
        self.assertEqual(single.structure_kind, ContentStructureKind.SINGLE_RESOURCE)
        self.assertEqual(single.members, (RepresentationMember(first, None, True),))
        descriptor = ImageSequenceDescriptor(1, 5, 2, 24000, 1001, (5, 1, 5))
        sequence = replace(value, content=ImageSequenceContent(first, descriptor))
        self.assertEqual(sequence.structure_kind, ContentStructureKind.IMAGE_SEQUENCE)
        self.assertIs(sequence.image_sequence, descriptor)
        self.assertEqual(descriptor.missing_frames, (1, 5))
        self.assertEqual(descriptor.rate, Fraction(24000, 1001))

    def test_compound_shapes_reject_invalid_membership(self) -> None:
        resource = ResourceId(UUID(int=1))
        required = RepresentationMember(resource, "example:essence", True)
        optional = replace(required, required=False)
        for factory in [OrderedPartsContent, PackageContent]:
            for members in [(), (required, required), (optional,)]:
                with self.subTest(factory=factory, members=members):
                    with self.assertRaises(ValueError):
                        factory(members)
        with self.assertRaises(ValueError):
            OrderedPartsContent(
                (
                    required,
                    RepresentationMember(
                        ResourceId(UUID(int=2)), "example:sidecar", False
                    ),
                )
            )
        with self.assertRaises(ValueError):
            PackageContent((RepresentationMember(resource, None, True),))
        with self.assertRaises(ValueError):
            RepresentationMember(resource, "unqualified", True)

    def test_sequence_descriptors_reject_unreachable_or_overflowing_values(
        self,
    ) -> None:
        valid = ImageSequenceDescriptor(1, 5, 2, 24, 1, ())
        for changes in [
            dict(end=4),
            dict(step=0),
            dict(start=-(2**63) - 1),
            dict(rate_denominator=0),
            dict(rate_numerator=2**32),
            dict(missing_frames=(2,)),
        ]:
            with self.subTest(changes=changes):
                with self.assertRaises(ValueError):
                    replace(valid, **changes)
        with self.assertRaises(TypeError):
            replace(valid, step=True)
