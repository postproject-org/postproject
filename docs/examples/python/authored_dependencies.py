"""A toy scene references a texture; the rendered output records provenance."""

import json
import sys
from pathlib import Path

from postproject import (
    ActivityEdge,
    ActivitySpec,
    ArtifactKnowledgeState,
    AssetRef,
    Dependency,
    DependencySetStatus,
    Production,
    RepresentationKind,
    ToolIdentity,
)


def exercise(work: Path) -> None:
    # This fixture format declares its complete references in a JSON array.
    # It demonstrates extraction, not a parser for a real scene/media format.
    texture = work / "texture.json"
    scene = work / "scene.json"
    output = work / "render.bin"
    texture.write_text('{"references": [], "colour": "red"}\n')
    scene.write_text('{"references": ["texture.json"]}\n')
    output.write_bytes(b"fixture output produced from the red scene")
    with Production.create(work / "dependencies.pproj") as production:
        with production.edit() as edit:
            texture_asset = edit.import_media(texture)
            scene_asset = edit.import_media(scene)
            edit.commit()
        with production.read_session() as view:
            texture_matches = view.find_known_media(texture, limit=2)
            scene_matches = view.find_known_media(scene, limit=2)
            assert (
                texture_matches.next_cursor is None and len(texture_matches.items) == 1
            )
            assert scene_matches.next_cursor is None and len(scene_matches.items) == 1
            texture_id = texture_matches.items[0].representation_id
            scene_id = scene_matches.items[0].representation_id
            assert view.dependency_set(scene_id) is None  # Never extracted.
            # [authored-dependency-extraction]
            authored = json.loads(scene.read_text())["references"]
            assert authored == ["texture.json"]
            assert json.loads(texture.read_text())["references"] == []
            with view.edit() as edit:
                edit.record_dependency_set(texture_id, ())  # Inspected, no references.
                edit.record_dependency_set(
                    scene_id,
                    tuple(
                        Dependency(
                            "example:texture",
                            AssetRef(texture_asset),
                            text,
                            resolved_representation_id=texture_id,
                        )
                        for text in authored
                    ),
                )
                edit.commit()
            # [/authored-dependency-extraction]
        with production.read_session() as view, view.edit() as edit:
            render_id = edit.add_representation(
                scene_asset, RepresentationKind.DERIVED, output
            )
            edit.create_activity(
                ActivitySpec(
                    "example:fixture-render",
                    inputs=(ActivityEdge(scene_id),),
                    outputs=(ActivityEdge(render_id),),
                    tool=ToolIdentity("Fixture renderer", "1.0"),
                )
            )
            edit.commit()
        assert (
            production.dependency_set(render_id) is None
        )  # Inputs are not authored references.
        assert (
            production.evaluate_artifact(render_id).state
            is ArtifactKnowledgeState.CURRENT
        )
        texture.write_text('{"references": [], "colour": "blue"}\n')
        with production.read_session() as view, view.edit() as edit:
            edit.observe_resource_content(texture_matches.items[0].resource_id, texture)
            edit.commit()
        changed = production.dependency_set(texture_id)
        assert (
            changed is not None
            and changed.status is DependencySetStatus.NEEDS_EXTRACTION
        )
        evaluation = production.evaluate_artifact(render_id)
        assert evaluation.state is ArtifactKnowledgeState.STALE
        assert evaluation.reasons  # Explain the changed texture through the scene.


if __name__ == "__main__":
    exercise(Path(sys.argv[1]))
