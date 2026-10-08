"""The source inventory must fail closed when tables or commands change."""

import shutil
import tempfile
import unittest
from pathlib import Path

from tools.check_exchange_inventory import ROOT, problems


class ExchangeInventoryTests(unittest.TestCase):
    def test_current_source_is_classified(self):
        self.assertEqual(problems(), [])

    def test_new_tables_columns_and_operations_require_review(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = [
                "docs/exchange-inventory.json",
                "crates/postproject-core/src/storage.rs",
                "crates/postproject-storage-sqlite/src/migrations",
            ]
            for relative in paths:
                source = ROOT / relative
                destination = root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                if source.is_dir():
                    shutil.copytree(source, destination)
                else:
                    shutil.copyfile(source, destination)
            migration = root / paths[-1] / "999_unreviewed.sql"
            migration.write_text(
                "CREATE TABLE unreviewed (id BLOB);"
                "ALTER TABLE assets ADD COLUMN unreviewed TEXT;"
            )
            storage = root / paths[1]
            storage.write_text(
                storage.read_text().replace(
                    "    fn rollback(&mut self)",
                    "    fn new_mutation(&mut self);\n    fn rollback(&mut self)",
                    1,
                )
            )
            self.assertEqual(
                problems(root),
                [
                    "unreviewed columns: assets",
                    "unclassified table: unreviewed",
                    "unreviewed transaction operation: new_mutation",
                ],
            )


if __name__ == "__main__":
    unittest.main()
