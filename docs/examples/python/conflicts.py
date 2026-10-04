"""Run the Python listing in the semantic-conflicts guide."""

from __future__ import annotations

import sys
from pathlib import Path

from postproject import ConflictError, ConflictKeyKind, Production


# [semantic-conflicts]
def update_from_a_base_revision(production: Production) -> None:
    with production.transaction() as setup:
        root_id = setup.add_media_root("rushes")

    base = production.latest_revision
    assert base is not None
    with production.transaction(base_revision=base.id) as first_writer:
        first_writer.set_media_root_enabled(root_id, False)
        receipt = first_writer.commit()

    assert receipt.production_id == production.id
    superseding = receipt.revision
    assert superseding is not None
    stale_writer = production.transaction(base_revision=base.id)
    try:
        stale_writer.set_media_root_enabled(root_id, True)
        stale_writer.commit()
    except ConflictError as error:
        conflict = error.conflict
        assert conflict is not None
        assert conflict.key.kind is ConflictKeyKind.MEDIA_ROOT
        assert conflict.key.target == root_id
        assert conflict.base_revision_id == base.id
        assert conflict.superseding_revision_id == superseding.id
    else:
        raise AssertionError("stale write did not conflict")
    finally:
        stale_writer.close()

    # Retry only after re-reading and deciding that enabling is still right.
    refreshed = production.latest_revision
    assert refreshed is not None
    with production.transaction(base_revision=refreshed.id) as retry:
        retry.set_media_root_enabled(root_id, True)


# [/semantic-conflicts]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: conflicts.py WORK_DIRECTORY")
    path = Path(sys.argv[1]) / "conflicts.pproj"
    with Production.create(path) as production:
        update_from_a_base_revision(production)


if __name__ == "__main__":
    main()
