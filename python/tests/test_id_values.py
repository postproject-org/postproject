"""Public identities retain standard UUID behavior and checked boundaries."""

import ctypes
import sqlite3
import tempfile
import unittest
from pathlib import Path
from typing import Any
from uuid import UUID

from postproject import (
    AssetId,
    AssetRef,
    MediaRootId,
    NotFoundError,
    Production,
    StorageError,
    _abi,
    _native,
    parse_id,
)


class IdentityTests(unittest.TestCase):
    def test_native_root_mutations_reject_other_id_structures(self) -> None:
        native = _native.NativeLibrary()
        error = ctypes.POINTER(_abi.Error)()
        for wrong_id in (_abi.Uuid(), _abi.AssetId(), _abi.ProductionId()):
            with self.assertRaises(ctypes.ArgumentError):
                native.lib.pp_transaction_set_media_root_enabled(
                    None, wrong_id, 0, ctypes.byref(error)
                )
            with self.assertRaises(ctypes.ArgumentError):
                native.lib.pp_transaction_remove_media_root(
                    None, wrong_id, ctypes.byref(error)
                )

    def test_root_mutations_validate_existence_and_production_scope(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                Production.create(root / "first.pproj") as first,
                Production.create(root / "second.pproj") as second,
            ):
                with first.transaction() as edit:
                    root_id = edit.add_media_root("rushes")
                    edit.commit()
                self.assertIsInstance(root_id, UUID)
                self.assertEqual(parse_id(str(root_id), MediaRootId), root_id)
                with first.read_session() as view:
                    self.assertEqual(view.media_roots[0].id, root_id)
                for identifier in (root_id, MediaRootId(UUID(int=0))):
                    for remove in (False, True):
                        with second.read_session() as view, view.edit() as edit:
                            if remove:
                                edit.remove_media_root(identifier)
                            else:
                                edit.set_media_root_enabled(identifier, False)
                            with self.assertRaises(NotFoundError):
                                edit.commit()
                        self.assertIsNone(second.latest_revision)
                        self.assertEqual(second.media_roots, ())
                self.assertTrue(first.media_roots[0].enabled)

    def test_asset_point_queries_do_not_materialize_unrelated_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "production.pproj"
            with Production.create(path):
                pass
            empty = AssetId(UUID(int=1))
            corrupt = AssetId(UUID(int=2))
            absent = AssetId(UUID(int=3))
            # SQLite's affinity permits this malformed timestamp. The fixture
            # proves a point query never decodes an unrelated asset row.
            with sqlite3.connect(path) as connection:
                connection.executemany(
                    "INSERT INTO assets(id, created_at_micros) VALUES (?, ?)",
                    [(empty.bytes, 0), (corrupt.bytes, "invalid timestamp")],
                )
            with Production.open(path) as production:
                self.assertIn(empty, production.assets)
                self.assertNotIn(absent, production.assets)
                self.assertNotIn(AssetId(UUID(int=0)), production.assets)
                self.assertEqual(production.resolve(empty), ())
                with self.assertRaises(NotFoundError):
                    production.resolve(absent)
                for query in (
                    lambda: corrupt in production.assets,
                    lambda: production.resolve(corrupt),
                    lambda: list(production.assets),
                ):
                    with self.assertRaises(StorageError):
                        query()

    def test_native_journal_arguments_reject_other_id_structures(self) -> None:
        native = _native.NativeLibrary()
        events = ctypes.POINTER(_abi.RevisionEventSet)()
        error = ctypes.POINTER(_abi.Error)()
        for wrong_id in (_abi.Uuid(), _abi.TransactionId(), _abi.ProductionId()):
            with self.assertRaises(ctypes.ArgumentError):
                native.lib.pp_production_revision_events(
                    None, wrong_id, ctypes.byref(events), ctypes.byref(error)
                )

    def test_native_asset_reads_reject_other_id_structures(self) -> None:
        native = _native.NativeLibrary()
        assets = ctypes.POINTER(_abi.AssetSet)()
        error = ctypes.POINTER(_abi.Error)()
        for wrong_id in (_abi.Uuid(), _abi.ProductionId(), _abi.RevisionId()):
            with self.assertRaises(ctypes.ArgumentError):
                native.lib.pp_production_asset(
                    None, wrong_id, ctypes.byref(assets), ctypes.byref(error)
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

    def test_assets_are_scoped_to_live_and_retained_production_reads(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            media = root / "media.dat"
            media.write_bytes(b"scoped assets")
            with (
                Production.create(root / "first.pproj") as first,
                Production.create(root / "second.pproj") as second,
            ):
                with first.transaction() as edit:
                    asset = edit.import_media(media)
                    edit.commit()
                with second.read_session() as view:
                    for reader in (second, view):
                        with self.assertRaises(NotFoundError):
                            reader.asset(asset)
                        with self.assertRaises(NotFoundError):
                            reader.asset(AssetId(UUID(int=0)))
                self.assertEqual(first.asset(asset).id, asset)
                with first.read_session() as view:
                    self.assertEqual(view.asset(asset).id, asset)

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
