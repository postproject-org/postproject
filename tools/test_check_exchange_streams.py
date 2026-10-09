"""Check independent stream framing beyond comparison with fixed JSON bytes."""

import copy
import json
import unittest

from tools.check_exchange_vectors import ROOT, canonical, check_stream


class StreamChecks(unittest.TestCase):
    def setUp(self):
        directory = ROOT / "tests/fixtures/exchange"
        self.vector = json.loads((directory / "stream-vectors.json").read_text())[0]
        self.stream = json.loads((directory / self.vector["file"]).read_text())

    def verify_altered(self, change):
        vector = copy.deepcopy(self.vector)
        stream = copy.deepcopy(self.stream)
        change(stream)
        # Deliberately update expected bytes, so rejection must come from the
        # independent ordering, scope or body checks rather than stale goldens.
        vector["canonical_manifest"] = canonical(
            json.dumps(stream["manifest"])
        ).decode()
        vector["canonical_chunks"] = [
            canonical(json.dumps(chunk)).decode() for chunk in stream["chunks"]
        ]
        with self.assertRaises((AssertionError, ValueError)):
            check_stream(vector, stream)

    def test_fixed_frame_and_utf8_splits(self):
        check_stream(self.vector, self.stream)
        self.assertEqual(len(self.stream["chunks"]), 3)

    def test_first_index_cannot_skip_zero(self):
        self.verify_altered(lambda stream: stream["chunks"][0].update(index="1"))

    def test_later_chunk_must_bind_its_immediate_predecessor(self):
        self.verify_altered(lambda stream: stream["chunks"][1].update(previous=None))

    def test_production_history_and_revision_cannot_be_rebound(self):
        for field in ["production", "history", "revision"]:
            with self.subTest(field=field):
                self.verify_altered(
                    lambda stream, field=field: stream["chunks"][0].update(
                        {field: "00000000-0000-4000-8000-000000000999"}
                    )
                )

    def test_body_has_exact_framed_content(self):
        self.verify_altered(lambda stream: stream["body"][0].update(values="2"))

    def test_nonzero_base64_padding_bits_reject(self):
        def alter(stream):
            chunk = stream["chunks"][0]
            self.assertTrue(chunk["payload"].endswith("A=="))
            chunk["payload"] = chunk["payload"][:-3] + "B=="

        self.verify_altered(alter)

    def test_manifest_count_and_size_are_exact(self):
        for field in ["count", "payload_bytes"]:
            with self.subTest(field=field):
                self.verify_altered(
                    lambda stream, field=field: stream["manifest"]["chunks"].update(
                        {field: "999"}
                    )
                )


if __name__ == "__main__":
    unittest.main()
