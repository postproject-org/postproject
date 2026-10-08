"""Reject unclassified schema relations and native transaction operations."""

from __future__ import annotations

import json
import re
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def problems(root: Path = ROOT) -> list[str]:
    inventory = json.loads((root / "docs/exchange-inventory.json").read_text())
    result = []
    tables = {item["table"]: item for item in inventory["tables"]}
    with sqlite3.connect(":memory:") as connection:
        migrations = root / "crates/postproject-storage-sqlite/src/migrations"
        for migration in sorted(migrations.glob("*.sql")):
            connection.executescript(migration.read_text())
        actual = {
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_schema WHERE type='table'"
            )
        }
        for name in sorted(actual | tables.keys()):
            if name not in tables:
                result.append(f"unclassified table: {name}")
                continue
            if name not in actual:
                result.append(f"inventory table absent: {name}")
                continue
            item = tables[name]
            if item["classification"] not in {"portable", "derived", "private"}:
                result.append(f"invalid table classification: {name}")
            columns = [
                row[1]
                for row in connection.execute(f'PRAGMA table_info("{name}")')
            ]
            if columns != item["columns"]:
                result.append(f"unreviewed columns: {name}")
            if not set(item["exclude"]).issubset(columns):
                result.append(f"unknown excluded columns: {name}")
    storage = (root / "crates/postproject-core/src/storage.rs").read_text()
    trait = storage.split("pub trait ProductionStoreTransaction")[1].split(
        "pub trait ProductionStore:"
    )[0]
    actual_operations = set(re.findall(r"^    fn (\w+)\(", trait, re.MULTILINE))
    operations = {
        item["operation"]: item for item in inventory["transaction_operations"]
    }
    for name in sorted(actual_operations ^ operations.keys()):
        result.append(f"unreviewed transaction operation: {name}")
    for item in operations.values():
        if item["classification"] not in {"lifecycle", "mutation"}:
            result.append(f"invalid operation classification: {item['operation']}")
    return result


def main() -> int:
    result = problems()
    for problem in result:
        print(problem)
    if not result:
        print("schema relations, columns and transaction operations classified")
    return bool(result)


if __name__ == "__main__":
    raise SystemExit(main())
