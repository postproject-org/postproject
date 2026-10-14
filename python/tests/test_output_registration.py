"""One output task preserves identity, parameters and all-or-nothing staging."""

import os
import tempfile
import unittest
from pathlib import Path
from uuid import uuid4

from postproject import (
    ActivityRef,
    ActivitySpec,
    CommitReceipt,
    FileResourceInput,
    IoError,
    MetadataU64,
    PackageSource,
    Production,
    RepresentationKind,
    ToolIdentity,
)
from postproject.exchange import ClientId, RequestId

LIBRARY = os.environ.get("POSTPROJECT_LIBRARY")


class OutputRegistrationTests(unittest.TestCase):
    def test_generation_returns_both_ids_and_actual_activity_parameters(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "render.dat"
            output.write_bytes(b"produced fixture pixels")
            with Production.create(
                root / "production.pproj", library_path=LIBRARY
            ) as production:
                with production.edit() as edit:
                    registered = edit.register_outputs(
                        ((None, RepresentationKind.ORIGINAL, output),),
                        ActivitySpec(
                            "example:generate",
                            tool=ToolIdentity("Fixture generator"),
                            parameters={("example:recipe", "setting"): 24},
                        ),
                    )
                    self.assertFalse(
                        production.asset_exists(registered.outputs[0].asset_id)
                    )
                    edit.commit()
                produced = registered.outputs[0]
                self.assertEqual(
                    production.representation(produced.representation_id).asset_id,
                    produced.asset_id,
                )
                activity = production.activities_producing[produced.representation_id][
                    0
                ]
                self.assertEqual(activity.id, registered.activity_id)
                self.assertEqual(activity.inputs, ())
                values = production.metadata[ActivityRef(activity.id)]
                self.assertEqual(values[0].value, (24))
                self.assertIsNone(production.dependency_set(produced.representation_id))

    def test_existing_identity_compound_outputs_roles_and_repeated_parameters(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output, sidecar = (
                root / name for name in ("source.dat", "render.dat", "sidecar.txt")
            )
            source.write_bytes(b"source")
            output.write_bytes(b"render")
            sidecar.write_bytes(b"output sidecar")
            with Production.create(
                root / "production.pproj", library_path=LIBRARY
            ) as production:
                with production.edit() as edit:
                    imported = edit.import_media(source)
                    edit.commit()
                package = PackageSource(
                    (
                        FileResourceInput(str(output), "example:main"),
                        FileResourceInput(str(sidecar), "example:sidecar", False),
                    )
                )
                with production.edit() as edit:
                    registered = edit.register_outputs(
                        (
                            (
                                imported.asset_id,
                                RepresentationKind.DERIVED,
                                package,
                                "example:master",
                            ),
                            (imported.asset_id, RepresentationKind.PROXY, output),
                        ),
                        ActivitySpec(
                            "example:render",
                            inputs=(imported.representation_id,),
                            parameters=(
                                (("example:recipe", "setting"), 1),
                                (("example:recipe", "setting"), 2),
                            ),
                        ),
                    )
                    edit.commit()
                self.assertEqual(len(registered.outputs), 2)
                self.assertTrue(
                    all(
                        value.asset_id == imported.asset_id
                        for value in registered.outputs
                    )
                )
                self.assertEqual(
                    len(
                        production.representation(
                            registered.outputs[0].representation_id
                        ).resources
                    ),
                    2,
                )
                activity = production.activities_producing[
                    registered.outputs[1].representation_id
                ][0]
                self.assertEqual(
                    activity.inputs[0].representation_id, imported.representation_id
                )
                self.assertEqual(
                    {edge.role for edge in activity.outputs}, {None, "example:master"}
                )
                self.assertEqual(len(production.metadata[ActivityRef(activity.id)]), 2)

    def test_output_task_captures_complete_proposal_and_replays_without_media_io(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / "render.dat"
            file.write_bytes(b"produced output")
            with Production.create(
                root / "authority.pproj", library_path=LIBRARY
            ) as authority:
                saved = authority.export_checkpoint(root / "checkpoint")
                with authority.edit() as edit:
                    registered = edit.register_outputs(
                        ((None, RepresentationKind.ORIGINAL, file),),
                        ActivitySpec(
                            "unknown:Exact-generation",
                            parameters=(
                                (("unknown:Recipe", "exact"), MetadataU64(2**64 - 1)),
                                (("unknown:Recipe", "exact"), b"opaque\x00bytes"),
                            ),
                        ),
                    )
                    proposal = edit.prepare_proposal(
                        ClientId(uuid4()), RequestId(uuid4())
                    )
                self.assertEqual(authority.activities, ())
                with proposal:
                    accepted = authority.submit_proposal(proposal)
                    self.assertIsInstance(accepted.outcome.status, CommitReceipt)
                    again = authority.submit_proposal(proposal)
                    self.assertEqual(again.outcome.document, accepted.outcome.document)
                through = authority.export_changes(saved, root / "changes")
                file.unlink()  # Catch-up replays recorded facts, never fingerprints files.
                with Production.import_checkpoint(
                    root / "checkpoint", root / "mirror.pproj", library_path=LIBRARY
                ) as mirror:
                    self.assertEqual(mirror.apply_changes(root / "changes"), through)
                    self.assertEqual(mirror.activities, authority.activities)
                    target = ActivityRef(registered.activity_id)
                    self.assertEqual(
                        mirror.metadata[target], authority.metadata[target]
                    )
                    self.assertEqual(len(mirror.metadata[target]), 2)
                    output = registered.outputs[0]
                    self.assertEqual(
                        mirror.representation(output.representation_id).asset_id,
                        output.asset_id,
                    )

    def test_late_media_failure_preserves_an_open_unchanged_edit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "render.dat"
            output.write_bytes(b"produced")
            with Production.create(
                root / "production.pproj", library_path=LIBRARY
            ) as production:
                with production.edit() as edit:
                    with self.assertRaises(IoError):
                        edit.register_outputs(
                            (
                                (None, RepresentationKind.ORIGINAL, output),
                                (None, RepresentationKind.ORIGINAL, root / "missing"),
                            ),
                            ActivitySpec("example:render"),
                        )
                    self.assertIsNone(edit.commit().revision)
                self.assertEqual(production.activities, ())


if __name__ == "__main__":
    unittest.main()
