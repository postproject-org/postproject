"""Reject reauthored checkpoint framing errors beyond stale golden bytes."""

import copy
import json
import unittest

from tools.check_exchange_vectors import ROOT, canonical, check_checkpoint


class CheckpointChecks(unittest.TestCase):
    def setUp(self):
        directory = ROOT / "tests/fixtures/exchange"
        self.vector = json.loads((directory / "checkpoint-vectors.json").read_text())[0]
        self.snapshot = json.loads((directory / self.vector["file"]).read_text())

    def altered(self, change):
        vector, snapshot = copy.deepcopy(self.vector), copy.deepcopy(self.snapshot)
        change(snapshot)
        vector["canonical_manifest"] = canonical(
            json.dumps(snapshot["manifest"])
        ).decode()
        vector["canonical_chunks"] = [
            canonical(json.dumps(chunk)).decode() for chunk in snapshot["chunks"]
        ]
        with self.assertRaises((AssertionError, ValueError)):
            check_checkpoint(vector, snapshot, False)

    def test_fixed_complete_genesis_and_split_scalar(self):
        check_checkpoint(self.vector, self.snapshot, False)
        self.assertEqual(len(self.snapshot["manifest"]["sections"]), 19)
        self.assertEqual(len(self.snapshot["chunks"]), 5)

    def test_reordered_sections_and_unsupported_features(self):
        def reorder(snapshot):
            sections = snapshot["manifest"]["sections"]
            sections[1], sections[2] = sections[2], sections[1]

        self.altered(reorder)
        self.altered(
            lambda snapshot: snapshot["manifest"].update(
                required_features=["future.v1"]
            )
        )

    def test_scoped_chunks_cannot_be_rebound_or_reordered(self):
        for field in (
            "checkpoint",
            "production",
            "history",
            "section",
            "index",
            "previous",
        ):
            with self.subTest(field=field):
                self.altered(
                    lambda snapshot, field=field: snapshot["chunks"][0].update(
                        {field: "foreign"}
                    )
                )

    def test_body_and_section_counts_are_exact(self):
        self.altered(
            lambda snapshot: snapshot["bodies"]["production"][0].update(
                display_name="changed"
            )
        )
        self.altered(
            lambda snapshot: snapshot["manifest"]["sections"][0].update(items="2")
        )
        self.altered(
            lambda snapshot: snapshot["manifest"]["sections"][0]["chunks"].update(
                payload_bytes="1"
            )
        )

    def test_missing_and_extra_chunks_cannot_match_the_manifest(self):
        self.altered(lambda snapshot: snapshot["chunks"].pop())
        self.altered(lambda snapshot: snapshot["chunks"].append(snapshot["chunks"][-1]))

    def test_nonempty_record_continuations_count_as_one_logical_item(self):
        directory = ROOT / "tests/fixtures/exchange"
        self.vector = next(
            vector
            for vector in json.loads(
                (directory / "checkpoint-vectors.json").read_text()
            )
            if vector["file"] == "checkpoints/media.json"
        )
        self.snapshot = json.loads((directory / self.vector["file"]).read_text())
        check_checkpoint(self.vector, self.snapshot, False)
        self.assertNotEqual(
            self.snapshot["manifest"]["head"], self.snapshot["manifest"]["floor"]
        )
        self.assertEqual(len(self.snapshot["bodies"]["records"]), 4)

        def count_continuations(snapshot):
            next(
                section
                for section in snapshot["manifest"]["sections"]
                if section["section"] == "records"
            )["items"] = "4"

        self.altered(count_continuations)

    def test_head_scope_and_floor_order_are_exact(self):
        self.altered(
            lambda snapshot: snapshot["manifest"]["head"].update(history="foreign")
        )

        def future_floor(snapshot):
            snapshot["manifest"]["floor"] = dict(
                snapshot["manifest"]["floor"], sequence="1"
            )

        self.altered(future_floor)

    def test_activity_snapshots_are_continuations_of_one_logical_item(self):
        directory = ROOT / "tests/fixtures/exchange"
        self.vector = next(
            vector
            for vector in json.loads(
                (directory / "checkpoint-vectors.json").read_text()
            )
            if vector["file"] == "checkpoints/provenance.json"
        )
        self.snapshot = json.loads((directory / self.vector["file"]).read_text())
        check_checkpoint(self.vector, self.snapshot, False)
        self.assertEqual(len(self.snapshot["bodies"]["activities"]), 6)

        def count_snapshots(snapshot):
            next(
                section
                for section in snapshot["manifest"]["sections"]
                if section["section"] == "activities"
            )["items"] = "6"

        self.altered(count_snapshots)

    def test_job_inputs_are_continuations_of_their_headers(self):
        directory = ROOT / "tests/fixtures/exchange"
        self.vector = next(
            vector
            for vector in json.loads(
                (directory / "checkpoint-vectors.json").read_text()
            )
            if vector["file"] == "checkpoints/jobs.json"
        )
        self.snapshot = json.loads((directory / self.vector["file"]).read_text())
        check_checkpoint(self.vector, self.snapshot, False)
        self.assertEqual(len(self.snapshot["bodies"]["jobs"]), 10)

        def count_inputs(snapshot):
            next(
                section
                for section in snapshot["manifest"]["sections"]
                if section["section"] == "jobs"
            )["items"] = "10"

        self.altered(count_inputs)


if __name__ == "__main__":
    unittest.main()
