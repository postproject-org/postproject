"""Native metadata inputs preserve exact kinds, order and replay effects."""

import tempfile
import unittest
from fractions import Fraction
from pathlib import Path
from uuid import uuid4

from postproject import (
    AppendMetadataCommand,
    ClientId,
    MetadataLanguageString,
    MetadataProperty,
    MetadataStruct,
    MetadataStructField,
    MetadataU64,
    Production,
    ProductionRef,
    RequestId,
)


class MetadataInputTests(unittest.TestCase):
    def test_native_collections_copy_inputs_and_reject_cycles_without_partial_writes(
        self,
    ):
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "values.pproj") as production:
                target = ProductionRef(production.id)
                key = ("unknown:Exact", "list")
                value = ["first", True, Fraction(2, 3)]
                cyclic = ["prepared child"]
                cyclic.append(cyclic)
                with production.edit() as edit:
                    for invalid in (cyclic, [0] * 4097, ["prepared child", object()]):
                        with self.assertRaises((TypeError, ValueError)):
                            edit.add_metadata(target, key, invalid)  # ty: ignore[invalid-argument-type]
                    edit.add_metadata(target, key, value)
                    value.append("changed after staging")
                    edit.commit()
                (assertion,) = production.metadata[target]
                self.assertEqual(assertion.value, ("first", True, Fraction(2, 3)))
                self.assertIsInstance(assertion.value, tuple)

    def test_equality_hashing_and_queries_keep_native_value_kinds_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            with Production.create(Path(directory) / "kinds.pproj") as production:
                target = ProductionRef(production.id)
                key = ("unknown:Exact", "kind")
                values = (True, 1, Fraction(1, 1), MetadataU64(1))
                with production.edit() as edit:
                    for value in values:
                        edit.add_metadata(target, key, value)
                        edit.add_metadata(
                            target,
                            ("unknown:Exact", "nested"),
                            MetadataStruct((MetadataStructField("same", (value,)),)),
                        )
                    edit.commit()
                scalar = production.metadata_by_property[key]
                nested = production.metadata_by_property[("unknown:Exact", "nested")]
                self.assertEqual(len(set(scalar)), 4)
                self.assertEqual(len(set(nested)), 4)
                self.assertEqual(
                    [type(item.value) for item in scalar],
                    [bool, int, Fraction, MetadataU64],
                )
                for value in values:
                    self.assertEqual(
                        len(
                            production.query_metadata(key, limit=10, value=value).items
                        ),
                        1,
                    )

    def test_native_scalars_keep_types_across_repeated_assertions(self) -> None:
        cases = [
            ("https://example.org/text", ("https://example.org/text")),
            (True, (True)),
            (False, (False)),
            (-(2**63), (-(2**63))),
            (2**63 - 1, (2**63 - 1)),
            (b"\0\xff", (b"\0\xff")),
            (Fraction(-3, 7), Fraction(-3, 7)),
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
                self.assertEqual(
                    [type(item.value) for item in stored],
                    [type(value) for _, value in cases for _ in range(2)],
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
                            "repeated", (("first", True, Fraction(2, 3)))
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
                            (
                                (
                                    ("first"),
                                    (True),
                                    Fraction(2, 3),
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
                    position = mirror.exchange_state.head
                    assert position is not None
                    production.export_changes(position, delta)
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
            {"x": "unsupported dictionary"},
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
                    [("retained")],
                )
