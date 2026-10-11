"""Native metadata inputs preserve exact kinds, order and replay effects."""

import tempfile
import unittest
from fractions import Fraction
from pathlib import Path
from uuid import uuid4

from postproject import (
    AppendMetadataCommand,
    ClientId,
    MetadataBool,
    MetadataBytes,
    MetadataI64,
    MetadataLanguageString,
    MetadataList,
    MetadataProperty,
    MetadataRational,
    MetadataString,
    MetadataStruct,
    MetadataStructField,
    MetadataU64,
    Production,
    ProductionRef,
    RequestId,
)


class MetadataInputTests(unittest.TestCase):
    def test_native_and_tagged_inputs_have_identical_copied_values(self) -> None:
        cases = [
            ("https://example.org/text", MetadataString("https://example.org/text")),
            (True, MetadataBool(True)),
            (False, MetadataBool(False)),
            (-(2**63), MetadataI64(-(2**63))),
            (2**63 - 1, MetadataI64(2**63 - 1)),
            (b"\0\xff", MetadataBytes(b"\0\xff")),
            (Fraction(-3, 7), MetadataRational(-3, 7)),
        ]
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "metadata.pproj") as production:
                target = ProductionRef(production.id)
                key = ("unknown:Exact名", "values")
                with production.edit() as edit:
                    for native, tagged in cases:
                        edit.add_metadata(target, key, native)
                        edit.add_metadata(target, MetadataProperty(*key), tagged)
                    edit.commit()
                stored = production.metadata[target]
                self.assertEqual(
                    [v.value for v in stored], [v for _, v in cases for _ in range(2)]
                )
                self.assertTrue(
                    all(v.property == MetadataProperty(*key) for v in stored)
                )
                self.assertEqual(production.metadata_by_property[key], stored)
                self.assertEqual(
                    len(production.query_metadata(key, limit=100, value=True).items), 2
                )
                self.assertEqual(
                    production.query_metadata(key, limit=100, value=1).items, ()
                )
                with production.read_session() as view:
                    self.assertEqual(view.metadata_by_property(key), stored)
                    self.assertEqual(
                        len(
                            view.query_metadata(
                                key, limit=100, value=Fraction(-3, 7)
                            ).items
                        ),
                        2,
                    )

    def test_nested_native_values_round_trip_through_proposal_and_delta(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with Production.create(root / "authority.pproj") as production:
                checkpoint = root / "checkpoint.ppck"
                production.export_checkpoint(checkpoint)
                target = ProductionRef(production.id)
                key = ("unknown:Ordered", "nested")
                value = MetadataStruct(
                    (
                        MetadataStructField(
                            "repeated", MetadataList(("first", True, Fraction(2, 3)))
                        ),
                        MetadataStructField("repeated", MetadataU64(2**64 - 1)),
                        MetadataStructField(
                            "language", MetadataLanguageString("名", "unknown-X")
                        ),
                    )
                )
                with production.proposal_builder(
                    ClientId(uuid4()), RequestId(uuid4())
                ) as builder:
                    builder.stage([AppendMetadataCommand(target, key, value)])
                    proposal = builder.prepare()
                with proposal:
                    outcome = production.submit_proposal(proposal)
                    retried = production.submit_proposal(proposal)
                    self.assertEqual(outcome.outcome.document, retried.outcome.document)
                expected = MetadataStruct(
                    (
                        MetadataStructField(
                            "repeated",
                            MetadataList(
                                (
                                    MetadataString("first"),
                                    MetadataBool(True),
                                    MetadataRational(2, 3),
                                )
                            ),
                        ),
                        MetadataStructField("repeated", MetadataU64(2**64 - 1)),
                        MetadataStructField(
                            "language", MetadataLanguageString("名", "unknown-X")
                        ),
                    )
                )
                self.assertEqual(production.metadata[target][0].value, expected)
                with Production.import_checkpoint(
                    checkpoint, root / "mirror.pproj"
                ) as mirror:
                    delta = root / "changes.ppdelta"
                    production.export_changes(mirror.exchange_state.head, delta)
                    mirror.apply_changes(delta)
                    self.assertEqual(
                        mirror.metadata[target], production.metadata[target]
                    )
                with production.read_session() as view, view.edit() as edit:
                    edit.remove_metadata_property(target, key)
                    edit.commit()
                self.assertEqual(production.metadata[target], ())

    def test_invalid_native_inputs_do_not_stage_partial_assertions(self) -> None:
        values = (
            2**63,
            -(2**63) - 1,
            Fraction(2**63, 1),
            Fraction(1, 2**64),
            1.5,
            None,
            {"x": 1},
            ["x"],
        )
        keys = (
            "title",
            ("vocab",),
            ("vocab", "title", "extra"),
            ("vocab", 1),
            ["vocab", "title"],
        )
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "metadata.pproj") as production:
                target = ProductionRef(production.id)
                with production.edit() as edit:
                    for value in values:
                        with (
                            self.subTest(value=value),
                            self.assertRaises((TypeError, ValueError)),
                        ):
                            edit.add_metadata(target, ("vocab", "title"), value)  # ty: ignore[invalid-argument-type]
                    for key in keys:
                        with self.subTest(key=key), self.assertRaises(TypeError):
                            edit.add_metadata(target, key, "valid")  # ty: ignore[invalid-argument-type]
                    edit.add_metadata(target, ("vocab", "title"), "retained")
                    edit.commit()
                self.assertEqual(
                    [v.value for v in production.metadata[target]],
                    [MetadataString("retained")],
                )
