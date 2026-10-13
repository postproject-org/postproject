"""A toy scene references a texture; the rendered output records provenance."""

import json
import sys
from pathlib import Path

from postproject import (
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
            texture_import = edit.import_media(texture)
            scene_import = edit.import_media(scene)
            edit.commit()
        with production.read_session() as view:
            texture_id = texture_import.representation_id
            scene_id = scene_import.representation_id
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
                            AssetRef(texture_import.asset_id),
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
                scene_import.asset_id, RepresentationKind.DERIVED, output
            )
            edit.create_activity(
                ActivitySpec(
                    "example:fixture-render",
                    inputs=(scene_id,),
                    outputs=(render_id,),
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
            (resource,) = view.representation(texture_id).resources
            edit.observe_resource_content(resource.id, texture)
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
