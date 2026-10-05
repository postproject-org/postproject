"""Public identities retain standard UUID behavior and checked boundaries."""

import ctypes
import tempfile
import unittest
from pathlib import Path
from typing import Any
from uuid import UUID

from postproject import (
    AssetId,
    AssetRef,
    NotFoundError,
    Production,
    _abi,
    _native,
    parse_id,
)


class IdentityTests(unittest.TestCase):
    def test_native_journal_arguments_reject_other_id_structures(self) -> None:
        native = _native.NativeLibrary()
        events = ctypes.POINTER(_abi.RevisionEventSet)()
        error = ctypes.POINTER(_abi.Error)()
        for wrong_id in (_abi.Uuid(), _abi.TransactionId(), _abi.ProductionId()):
            with self.assertRaises(ctypes.ArgumentError):
                native.lib.pp_production_revision_events(
                    None, wrong_id, ctypes.byref(events), ctypes.byref(error)
                )

    def test_nominal_annotation_keeps_uuid_identity_and_standard_operations(
        self,
    ) -> None:
        raw = UUID(int=1)
        asset = AssetId(raw)
        self.assertIs(asset, raw)
        self.assertIsInstance(asset, UUID)
        values: dict[UUID, str] = {asset: "value"}
        self.assertEqual(values[raw], "value")
        self.assertEqual(parse_id(str(asset), AssetId), asset)
        with self.assertRaises(ValueError):
            parse_id("invalid", AssetId)

    def test_untyped_input_is_validated_without_claiming_runtime_id_kinds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            media = Path(directory) / "media.dat"
            media.write_bytes(b"public UUID boundary")
            with Production.create(path) as production:
                with production.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                self.assertIsInstance(asset, UUID)
                self.assertEqual(production.asset(asset).id, asset)
                untyped: Any = "invalid"
                with self.assertRaises(TypeError):
                    production.asset(untyped)
                with self.assertRaises(TypeError):
                    AssetRef(untyped)
                absent: Any = UUID(int=1)
                with self.assertRaises(NotFoundError):
                    production.asset(absent)
                representation: Any = production.representations[asset][0].id
                with self.assertRaises(NotFoundError):
                    production.asset(representation)
