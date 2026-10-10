"""Independent proposal framing and operation coverage for fixed wire vectors.

This uses only standard-library values, never the SDK or its schema generator.
Rust separately checks the same inputs with domain constructors.
"""

from __future__ import annotations

from uuid import UUID

FIELDS = {
    "media.import-original": "asset media",
    "representation.add": "media",
    "root.add": "root",
    "root.set-enabled": "id enabled",
    "root.remove": "id",
    "locator.add": "locator",
    "locator.retire": "id",
    "identifier.add": "attachment",
    "identifier.remove": "attachment",
    "resource.observe-file-facts": "resource",
    "resource.observe-fingerprint": "id fingerprint",
    "representation.observe-fingerprint": "id fingerprint",
    "metadata.append": "target property value",
    "metadata.replace": "target property values",
    "metadata.remove": "target property",
    "dependency.observe-set": "id occurrences",
    "activity.create": "header inputs outputs",
    "job.request": "header inputs",
    "job.claim": "id duration_micros tool agent",
    "job.renew": "id duration_micros",
    "job.release": "id",
    "job.fail": "id diagnostic",
    "job.complete": "id output activity",
    "job.cancel": "id",
}


def check_proposal(value: dict) -> set[str]:
    assert set(value) == {
        "kind",
        "version",
        "required_features",
        "production",
        "history",
        "client",
        "request",
        "base",
        "origin",
        "message",
        "commands",
        "extensions",
    }
    assert value["kind"] == "proposal" and value["version"] == "1"
    for name in ("production", "history", "client", "request"):
        assert str(UUID(value[name])) == value[name]
    assert len(value["commands"]) <= 1000
    features = set()
    operations = set()
    for command in value["commands"]:
        kind = command["kind"]
        assert set(command) == {"kind", *FIELDS[kind].split()}
        operations.add(kind)
        if kind.startswith("job."):
            features.add("jobs.v1")
        elif kind.startswith("metadata."):
            features.add("metadata.v1")
        elif kind.startswith("dependency."):
            features.add("dependencies.v1")
        elif kind.startswith("activity."):
            features.add("provenance.v1")
        else:
            features.add("media.v1")
        if kind == "job.complete":
            features.update(("media.v1", "provenance.v1"))
            assert command["activity"]["kind"] == "activity.create"
        if kind in ("job.claim", "job.renew"):
            micros = command["duration_micros"]
            assert str(int(micros)) == micros and 1 <= int(micros) <= 86_400_000_000
    assert value["required_features"] == sorted(features or {"metadata.v1"})
    return operations
