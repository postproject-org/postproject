"""Five ordinary host tasks; run with a fresh prepared WORK_DIRECTORY."""

from __future__ import annotations

import sys
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivityRef,
    ActivitySpec,
    AssetRef,
    EvidenceKind,
    Production,
    RepresentationKind,
    ResourceResolutionState,
    ResourceResolvedExact,
    ToolIdentity,
    VerificationMode,
    file_locator,
)


def exercise(work: Path) -> None:
    original = work / "rushes" / "A001.mov"
    moved = work / "moved" / "A001.mov"
    output = work / "renders" / "preview.mov"
    # The example fixture supplies the original. These are demonstration bytes,
    # not a media transcoder; the host must produce the real output itself.
    output.write_bytes(original.read_bytes())

    # [ergonomic-import]
    with Production.create(work / "tasks.pproj", "Documentary") as production:
        with production.edit(origin="example:editor", message="Import take") as edit:
            asset_id = edit.import_media(original, display_name="Camera A")
            edit.add_metadata(
                AssetRef(asset_id), ("example:editorial", "title"), "Interview"
            )
            receipt = edit.commit()
        assert receipt.revision is not None  # This edit's own revision.
        # [/ergonomic-import]

        # [ergonomic-adopt]
        with production.read_session() as view:
            matches = view.find_known_media(original, limit=2)
            if matches.next_cursor is not None or len(matches.items) != 1:
                raise ValueError("Adoption needs an explicit, unique choice")
            known = matches.items[0]
            adopted_asset = known.asset_id  # Store this ID in the host project.
        assert adopted_asset == asset_id  # No new identity or revision.
        # [/ergonomic-adopt]

        original.rename(moved)
        # [ergonomic-resolve]
        with production.read_session() as view:
            resolutions = view.resolve(
                asset_id,
                search_directories=[moved.parent],
                verification=VerificationMode.CONTENT,
            )
            base = view.decision_base
        # The view is closed before host/UI work. Never choose among ambiguity.
        selected = next(
            r for r in resolutions if r.representation_id == known.representation_id
        )
        resource = next(
            r for r in selected.resources if r.resource_id == known.resource_id
        )
        if resource.state is not ResourceResolutionState.RESOLVED_EXACT:
            raise ValueError("Relink needs a verified choice for the selected resource")
        assert isinstance(resource.outcome, ResourceResolvedExact)
        candidate = resource.outcome.candidate
        evidence = {e.kind for e in (*resource.evidence, *candidate.evidence)}
        if (
            EvidenceKind.SEARCH_TRUNCATED in evidence
            or EvidenceKind.FULL_HASH_MATCH not in evidence
        ):
            raise ValueError(
                "This file recipe needs complete search and full verification"
            )
        with production.edit(base, message="Confirm moved take") as edit:
            edit.confirm_locator(
                resource.resource_id,
                candidate.uri,
                media_root=candidate.media_root,
                sequence_naming=candidate.sequence_naming,
            )
            relink_receipt = edit.commit()
        assert relink_receipt.revision is not None
        # [/ergonomic-resolve]

        # [ergonomic-derived]
        with production.read_session() as view:
            input_id = known.representation_id  # Retained from explicit adoption.
            with view.edit(message="Record host output") as edit:
                output_id = edit.add_representation(
                    asset_id, RepresentationKind.DERIVED, output
                )
                activity_id = edit.create_activity(
                    ActivitySpec(
                        "example:preview",
                        inputs=(ActivityEdge(input_id),),
                        outputs=(ActivityEdge(output_id),),
                        tool=ToolIdentity("Example file copy", "1.0"),
                    )
                )
                edit.add_metadata(
                    ActivityRef(activity_id), ("example:preview", "mode"), "copy"
                )
                output_receipt = edit.commit()  # Output and knowledge are atomic.
        assert output_receipt.revision is not None
        # [/ergonomic-derived]

        # [ergonomic-revisions]
        with production.read_session() as view:
            assets = list(view.assets(page_size=100))
            revisions = list(view.revisions(after=0, page_size=100))
        assert len(assets) == 1
        assert [revision.sequence for revision in revisions] == [1, 2, 3]
        # [/ergonomic-revisions]
        assert (
            production.dependency_set(output_id) is None
        )  # No authored references observed.
        tool = production.activities_producing[output_id][0].tool
        assert tool is not None and tool.name == "Example file copy"
        locators = (
            production.representation(known.representation_id).resources[0].locators
        )
        assert file_locator(moved) in [locator.uri for locator in locators]


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: ergonomics.py WORK_DIRECTORY")
    exercise(Path(sys.argv[1]).resolve())
