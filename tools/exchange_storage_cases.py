"""Targeted rehashed mutations of independently authored storage fixtures."""

from __future__ import annotations

from collections.abc import Callable

Case = tuple[str, str, int, Callable[[dict], None]]


def first(frames: list[dict], kind: str) -> dict:
    return next(frame for frame in frames if frame["kind"] == kind)


def record_cases() -> list[Case]:
    def append_position(record):
        next(
            frame
            for frame in record["body"]
            if frame["kind"] == "metadata.effect" and frame["operation"] == "appended"
        )["position"] = "9"

    def prior_fingerprint(record):
        next(
            frame
            for frame in record["body"]
            if frame["kind"] == "fingerprint.change" and frame["previous"]
        )["previous"]["value"] = "AP8="

    def marker_revision(record):
        first(record["body"], "fingerprint.recomputation")["revision_sequence"] = "2"

    def occurrence_position(record):
        first(record["body"], "dependency.occurrence")["position"] = "1"

    def activity_fingerprint(record):
        first(record["body"], "fingerprint.snapshot")["fingerprint"]["value"] = "AP8="

    def activity_path(record):
        first(record["body"], "activity.dependency-segment")["occurrence"][
            "dependency"
        ]["authored_reference"] = "changed"

    def previous_claim(record):
        next(frame for frame in record["body"] if frame.get("operation") == "renew")[
            "previous"
        ]["expires_at_micros"] = "1000"

    def completion_boundary(record):
        first(record["body"], "job.transition")["input_boundary"] = "7"

    def capability_field(record):
        first(record["body"], "job.transition")["state"]["claim_id"] = (
            "00000000-0000-4000-8000-000000000001"
        )

    def future_effect(record):
        record["body"][0]["kind"] = "vendor:required-effect.v2"

    def foreign_scope(record):
        record["manifest"]["predecessor"]["history"] = (
            "00000000-0000-4000-8000-000000000001"
        )

    def forged_predecessor(record):
        record["manifest"]["predecessor"]["digest"] = "0" * 64

    def invalid_sequence_exception(record):
        first(record["body"], "structure.exception")["frame"] = "4"

    def optional_package(record):
        owner = next(
            frame["representation_id"]
            for frame in record["body"]
            if frame["kind"] == "structure.header"
            and frame["content"]["shape"] == "package"
        )
        for frame in record["body"]:
            if (
                frame["kind"] == "structure.member"
                and frame["representation_id"] == owner
            ):
                frame["required"] = False

    return [
        ("noncontiguous assertion", "metadata-edits", 18, append_position),
        ("false prior fingerprint", "knowledge", 18, prior_fingerprint),
        ("wrong marker revision", "knowledge", 18, marker_revision),
        ("reordered occurrence", "knowledge", 18, occurrence_position),
        ("invented activity fingerprint", "provenance", 18, activity_fingerprint),
        ("invented historical path", "provenance", 18, activity_path),
        ("false previous claim", "job-lifecycle", 18, previous_claim),
        ("future completion input boundary", "job-completion", 18, completion_boundary),
        ("credential field in inert claim", "job-lifecycle", 12, capability_field),
        ("unsupported required effect", "metadata-edits", 10, future_effect),
        ("foreign history", "metadata-edits", 14, foreign_scope),
        ("forged predecessor", "metadata-edits", 17, forged_predecessor),
        (
            "exception outside stepped domain",
            "compound",
            12,
            invalid_sequence_exception,
        ),
        ("late package without required essence", "compound", 12, optional_package),
    ]


def checkpoint_cases() -> list[Case]:
    def wrong_current_fingerprint(snapshot):
        first(snapshot["bodies"]["fingerprints"], "fingerprint.observation")[
            "fingerprint"
        ]["value"] = "AP8="

    def wrong_dependency_boundary(snapshot):
        snapshot["bodies"]["dependencies"][0]["recorded_revision_sequence"] = "3"

    def wrong_snapshot(snapshot):
        first(snapshot["bodies"]["activities"], "fingerprint.snapshot")["fingerprint"][
            "value"
        ] = "AP8="

    def missing_removed_guard(snapshot):
        frames = snapshot["bodies"]["conflict_versions"]
        frames[:] = [frame for frame in frames if frame["key"]["kind"] != "media_root"]
        next(
            section
            for section in snapshot["manifest"]["sections"]
            if section["section"] == "conflict_versions"
        )["items"] = str(len(frames))

    def wrong_job_state(snapshot):
        next(
            frame
            for frame in snapshot["bodies"]["jobs"]
            if frame["kind"] == "job.header" and frame["state"]["kind"] == "failed"
        )["state"]["diagnostic"] = "changed"

    def extra_capability(snapshot):
        next(
            frame
            for frame in snapshot["bodies"]["jobs"]
            if frame["kind"] == "job.header" and frame["state"]["kind"] == "claimed"
        )["state"]["claim_id"] = "00000000-0000-4000-8000-000000000001"

    def wrong_metadata(snapshot):
        snapshot["bodies"]["metadata"][0]["value"]["value"] = "changed"

    def forged_prior_anchor(snapshot):
        snapshot["bodies"]["archives"][0]["payload"] = (
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        )

    def archive_after_floor(snapshot):
        snapshot["bodies"]["archives"][-1]["sequence"] = "2"
        snapshot["bodies"]["archives"][-1]["revision"] = snapshot["manifest"]["head"][
            "revision"
        ]

    return [
        ("false current fingerprint", "knowledge", 18, wrong_current_fingerprint),
        ("false dependency boundary", "provenance", 18, wrong_dependency_boundary),
        ("false captured activity bytes", "provenance", 18, wrong_snapshot),
        ("missing removed guard", "access", 18, missing_removed_guard),
        ("false terminal diagnostic", "jobs", 18, wrong_job_state),
        ("credential field in public checkpoint", "jobs", 12, extra_capability),
        ("false current metadata", "metadata", 18, wrong_metadata),
        ("forged prior anchor", "archives", 18, forged_prior_anchor),
        ("archive after replay floor", "archives", 18, archive_after_floor),
    ]
