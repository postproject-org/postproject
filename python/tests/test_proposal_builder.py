"""Typed collections preserve exact domain state and prepared identities."""

import tempfile
import unittest
from itertools import repeat
from pathlib import Path
from uuid import uuid4

from postproject import (
    AddIdentifierCommand,
    AddMediaRootCommand,
    AddRepresentationCommand,
    AppendMetadataCommand,
    AssetRef,
    ClientId,
    CommitReceipt,
    ConfirmLocatorCommand,
    ExchangeError,
    ExchangeFailure,
    ExternalIdentifier,
    Fingerprint,
    ImportMediaCommand,
    InvalidArgumentError,
    MetadataProperty,
    MetadataString,
    MetadataU64,
    ObserveResourceContentCommand,
    PreparedAsset,
    PreparedMediaRoot,
    PreparedRepresentation,
    Production,
    RecordDependencySetCommand,
    RecordRepresentationFingerprintCommand,
    RecordResourceFingerprintCommand,
    RemoveIdentifierCommand,
    RemoveMediaRootCommand,
    RemoveMetadataCommand,
    ReplaceMetadataCommand,
    RepresentationKind,
    RequestId,
    RetireLocatorCommand,
    SetMediaRootEnabledCommand,
)


class ProposalBuilderTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.work = Path(directory.name)
        self.media = self.work / "media.dat"
        self.media.write_bytes(b"fixed native preparation fixture")
        self.production = Production.create(self.work / "authority.pproj")
        self.addCleanup(self.production.close)
        self.client = ClientId(uuid4())

    def test_prepared_ids_link_later_commands_and_values_remain_ordered(self) -> None:
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            imported = builder.stage([ImportMediaCommand(self.media)])[0]
            assert isinstance(imported, PreparedAsset)
            self.asset = imported.asset_id
            root = builder.stage([AddMediaRootCommand("rushes", "Label 名", -5)])[0]
            assert isinstance(root, PreparedMediaRoot)
            target = AssetRef(self.asset)
            property = MetadataProperty("unknown:Exact名", "values")
            identifier = ExternalIdentifier("unknown:ID", "Value名", "Q")
            builder.stage(
                [
                    AppendMetadataCommand(target, property, MetadataU64(2**64 - 1)),
                    AppendMetadataCommand(target, property, MetadataString("first")),
                    AddIdentifierCommand(target, identifier),
                ]
            )
            proposal = builder.prepare()
            with self.assertRaises(InvalidArgumentError):
                builder.stage([])
        with proposal:
            result = self.production.submit_proposal(proposal)
            self.assertIsInstance(result.outcome.status, CommitReceipt)
        self.assertEqual(self.production.asset(self.asset).id, self.asset)
        self.assertEqual(
            [item.value for item in self.production.metadata[target]],
            [MetadataU64(2**64 - 1), MetadataString("first")],
        )
        state = self.production.exchange_state
        with self.production.proposal_builder(
            self.client, RequestId(uuid4()), base=state.observed_base
        ) as builder:
            builder.stage(
                [
                    ReplaceMetadataCommand(
                        target, property, [MetadataString("second"), MetadataU64(9)]
                    ),
                    RemoveIdentifierCommand(target, identifier),
                    SetMediaRootEnabledCommand(root.root_id, False),
                ]
            )
            replacement = builder.prepare()
        with replacement:
            self.production.submit_proposal(replacement)
        self.assertEqual(
            [item.value for item in self.production.metadata[target]],
            [MetadataString("second"), MetadataU64(9)],
        )
        self.assertEqual(self.production.external_identifiers[target], ())
        self.assertFalse(self.production.media_roots[0].enabled)
        with self.production.read_session() as view:
            base = view.decision_base
        with self.production.proposal_builder(
            self.client, RequestId(uuid4()), base=base
        ) as builder:
            builder.stage(
                [
                    RemoveMetadataCommand(target, property),
                    RemoveMediaRootCommand(root.root_id),
                ]
            )
            removed = builder.prepare()
        with removed:
            self.production.submit_proposal(removed)
        self.assertEqual(self.production.media_roots, ())
        self.assertEqual(self.production.metadata[target], ())

    def test_prepared_evidence_uses_native_checks_and_copies_into_passive_store(
        self,
    ) -> None:
        with self.production.edit() as edit:
            asset = edit.import_media(self.media)
            edit.commit()
        original = self.production.representations[asset][0]
        resource = original.resources[0]
        state = self.production.exchange_state
        checkpoint, mirror_path = self.work / "checkpoint", self.work / "mirror.pproj"
        saved = self.production.export_checkpoint(checkpoint)
        with self.production.proposal_builder(
            self.client, RequestId(uuid4()), base=state.observed_base
        ) as builder:
            prepared = builder.stage(
                [AddRepresentationCommand(asset, RepresentationKind.PROXY, self.media)]
            )[0]
            assert isinstance(prepared, PreparedRepresentation)
            builder.stage(
                [
                    ConfirmLocatorCommand(
                        resource.id, "file:///exact-unknown-name.dat"
                    ),
                    RetireLocatorCommand(resource.locators[0].id),
                    RecordResourceFingerprintCommand(
                        resource.id, Fingerprint("EXACT_unknown", 7, b"\x00\xff")
                    ),
                    RecordRepresentationFingerprintCommand(
                        original.id, Fingerprint("EXACT_representation", 3, b"\xff\x00")
                    ),
                    RecordDependencySetCommand(prepared.representation_id, []),
                    ObserveResourceContentCommand(resource.id, self.media),
                ]
            )
            proposal = builder.prepare()
        with proposal:
            outcome = self.production.submit_proposal(proposal).outcome
            self.assertIsInstance(outcome.status, CommitReceipt)
        changes = self.work / "changes"
        through = self.production.export_changes(saved, changes)
        with Production.import_checkpoint(checkpoint, mirror_path) as mirror:
            self.assertEqual(mirror.apply_changes(changes), through)
            self.assertEqual(
                mirror.representations[asset], self.production.representations[asset]
            )
            self.assertEqual(
                mirror.dependency_set(prepared.representation_id),
                self.production.dependency_set(prepared.representation_id),
            )
        self.assertEqual(len(self.production.representations[asset]), 2)

    def test_bounded_collection_failure_and_discard_create_no_domain_revision(
        self,
    ) -> None:
        with self.production.proposal_builder(
            self.client, RequestId(uuid4())
        ) as builder:
            with self.assertRaises(ExchangeError) as bounded:
                builder.stage(repeat(AddMediaRootCommand("unbounded")))
            self.assertEqual(bounded.exception.kind, ExchangeFailure.LIMIT_EXCEEDED)
            builder.stage([AddMediaRootCommand("discarded")])
        self.assertEqual(self.production.media_roots, ())
        self.assertIsNone(self.production.latest_revision)


if __name__ == "__main__":
    unittest.main()
