"""Standard UUID values and checked portable continuation budgets."""

import unittest
from dataclasses import replace
from uuid import UUID, uuid4

from postproject import (
    ClientId,
    DecisionBase,
    ExchangeLimits,
    ExchangePosition,
    HistoryId,
    ProductionId,
    ProtocolBase,
    RequestId,
)


class ExchangeValueTests(unittest.TestCase):
    def test_identity_hints_preserve_standard_uuid_values(self) -> None:
        identifier = uuid4()
        for kind in (ClientId, RequestId, HistoryId):
            value = kind(identifier)
            self.assertIs(value, identifier)
            self.assertIsInstance(value, UUID)
        base = ProtocolBase(
            DecisionBase(ProductionId(uuid4()), None), HistoryId(identifier)
        )
        position = ExchangePosition(base, bytes(range(32)))
        self.assertEqual(position.digest, bytes(range(32)))
        self.assertIs(position.base, base)
        for digest in (b"", bytes(31), bytes(33), bytearray(32)):
            with self.assertRaises(ValueError):
                ExchangePosition(base, digest)  # ty: ignore[invalid-argument-type]

    def test_receiver_budgets_reject_zero_overflow_boolean_and_fraction(self) -> None:
        defaults = ExchangeLimits()
        for name in (
            "file_bytes",
            "disk_bytes",
            "frames",
            "document_bytes",
            "document_nodes",
        ):
            for invalid in (0, -1, 2**64, True, 1.5):
                with self.assertRaises(ValueError):
                    replace(defaults, **{name: invalid})
        for invalid in (0, 193, True, 1.5):
            with self.assertRaises(ValueError):
                replace(defaults, document_depth=invalid)  # type: ignore[arg-type]
        self.assertEqual(replace(defaults, file_bytes=2**64 - 1).file_bytes, 2**64 - 1)


if __name__ == "__main__":
    unittest.main()
