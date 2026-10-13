"""Register an already-produced render and thumbnail, then inspect their recipe."""

import sys
from pathlib import Path

from postproject import (
    ActivityRef,
    ActivitySpec,
    Production,
    RepresentationKind,
    ToolIdentity,
)


def exercise(work: Path) -> None:
    render, thumbnail = work / "render.dat", work / "thumbnail.dat"
    render.write_bytes(b"fixture render")
    thumbnail.write_bytes(b"fixture thumbnail")
    with Production.create(work / "outputs.pproj") as production:
        # [register-outputs]
        with production.edit(message="Record completed generation") as edit:
            registered = edit.register_outputs(
                (
                    (None, RepresentationKind.ORIGINAL, render),
                    (None, RepresentationKind.ORIGINAL, thumbnail, "example:thumbnail"),
                ),
                ActivitySpec(
                    "example:generate",
                    tool=ToolIdentity("Fixture generator", "1.0"),
                    parameters={("example:recipe", "width"): 1920},
                ),
            )
            receipt = edit.commit()
        # Inputs are empty because this operation generated new media.
        # For a render from recorded sources, pass their IDs in ActivitySpec.inputs.
        # [/register-outputs]
        assert receipt.revision is not None
        assert len(registered.outputs) == 2
        for output in registered.outputs:
            activity = production.activities_producing[output.representation_id][0]
            assert activity.id == registered.activity_id and not activity.inputs
            assert production.dependency_set(output.representation_id) is None
        assert len(production.metadata[ActivityRef(registered.activity_id)]) == 1


if __name__ == "__main__":
    exercise(Path(sys.argv[1]))
