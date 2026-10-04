"""Change delivery: filtered revision pages, waiters, and observers."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

from postproject import (
    AssetImportedEvent,
    JobSucceededEvent,
    MediaRootAddedEvent,
    Production,
    Revision,
    RevisionEvent,
    RevisionObserver,
    RevisionWaitResult,
)

LIBRARY_PATH = os.environ.get("POSTPROJECT_LIBRARY")
CLI_PATH = os.environ.get("POSTPROJECT_CLI")


class ChangeDeliveryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if LIBRARY_PATH is None:
            raise RuntimeError("POSTPROJECT_LIBRARY must name the native test library")

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.production_path = self.root / "changes.pproj"

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def _create(self) -> Production:
        return Production.create(self.production_path, library_path=LIBRARY_PATH)

    def test_filtered_pages_skip_unrelated_revisions(self) -> None:
        media = self.root / "A001.mov"
        media.write_bytes(b"filtered page fixture")
        with self._create() as production:
            with production.transaction() as transaction:
                transaction.import_media(media)
            with production.transaction() as transaction:
                transaction.add_media_root("media")
            page = production.changes_since_filtered(0, [MediaRootAddedEvent])
            self.assertEqual([revision.sequence for revision in page.revisions], [2])
            self.assertEqual(page.through_sequence, 2)
            first = production.changes_since_filtered(
                0, [AssetImportedEvent, MediaRootAddedEvent], limit=1
            )
            self.assertEqual(first.through_sequence, 1)
            with self.assertRaises(ValueError):
                production.changes_since_filtered(0, [Revision])  # ty: ignore[invalid-argument-type]

    def test_waiter_reports_revisions_timeouts_and_cancellation(self) -> None:
        with self._create() as production:
            with production.revision_waiter() as waiter:
                self.assertIs(
                    waiter.wait(0, timeout=0).result, RevisionWaitResult.TIMED_OUT
                )
                with production.transaction() as transaction:
                    transaction.add_media_root("media")
                wait = waiter.wait(0, timeout=0)
                self.assertIs(wait.result, RevisionWaitResult.REVISIONS)
                self.assertEqual(wait.revisions, production.changes_since(0, 10))
                with self.assertRaises(ValueError):
                    waiter.wait(0, timeout=61)
                waiter.cancel()
                self.assertIs(waiter.wait(1).result, RevisionWaitResult.CANCELLED)

    def test_closing_the_production_releases_a_blocked_waiter(self) -> None:
        production = self._create()
        waiter = production.revision_waiter()
        results: list[RevisionWaitResult] = []
        thread = threading.Thread(target=lambda: results.append(waiter.wait(0).result))
        thread.start()
        production.close()
        thread.join(timeout=120)
        self.assertEqual(results, [RevisionWaitResult.CLOSED])
        waiter.close()

    def test_observer_delivers_matching_revisions_on_its_thread(self) -> None:
        delivered: list[tuple[int, type]] = []
        received = threading.Event()

        def record(revision: Revision, events: tuple[RevisionEvent, ...]) -> None:
            delivered.append((revision.sequence, type(events[0].payload)))
            received.set()

        with self._create() as production:
            with RevisionObserver(
                production, record, kinds=[MediaRootAddedEvent]
            ) as observer:
                with production.transaction() as transaction:
                    transaction.add_media_root("media")
                self.assertTrue(received.wait(timeout=60))
            self.assertIsNone(observer.error)
            self.assertEqual(delivered, [(1, MediaRootAddedEvent)])
            self.assertEqual(observer.cursor, 1)

    @unittest.skipIf(CLI_PATH is None, "POSTPROJECT_CLI names no CLI executable")
    def test_waiting_process_observes_proxy_from_cli_job_run(self) -> None:
        source = self.root / "source.mov"
        source.write_bytes(b"source media")
        proxies = self.root / "proxies"
        proxies.mkdir()
        production = str(self.production_path)
        self._cli("init", production)
        self._cli("root", "add", production, "proxies")
        imported = self._cli("media", "add", production, str(source))
        requested = self._cli(
            "job",
            "request",
            production,
            "org.postproject:generate-proxy",
            imported["asset_id"],
            "proxy",
            "--input",
            imported["representation_id"],
            "--target-root",
            "proxies",
            "--profile",
            "proxy-720p",
        )

        with Production.open(
            self.production_path, library_path=LIBRARY_PATH
        ) as waiting:
            latest = waiting.latest_revision
            assert latest is not None
            cursor = latest.sequence
            with waiting.revision_waiter() as waiter:
                worker = subprocess.Popen(
                    [
                        str(CLI_PATH),
                        "--json",
                        "job",
                        "run",
                        production,
                        "--once",
                        "--root-map",
                        f"proxies={proxies}",
                        "--ffmpeg",
                        str(_fake_ffmpeg(self.root)),
                    ],
                    stdout=subprocess.DEVNULL,
                )
                succeeded = None
                for _ in range(10):
                    if waiter.wait(cursor).result is not RevisionWaitResult.REVISIONS:
                        continue
                    page = waiting.changes_since_filtered(cursor, [JobSucceededEvent])
                    cursor = page.through_sequence
                    if page.revisions:
                        succeeded = page.revisions[0]
                        break
                self.assertEqual(worker.wait(timeout=120), 0)
            self.assertIsNotNone(succeeded)
            assert succeeded is not None
            payloads = [
                event.payload for event in waiting.revision_events[succeeded.id]
            ]
            job_ids = [
                str(payload.job_id)
                for payload in payloads
                if isinstance(payload, JobSucceededEvent)
            ]
            self.assertEqual(job_ids, [requested["id"]])
            self.assertEqual(len(list(proxies.iterdir())), 1)

    def _cli(self, *arguments: str) -> dict:
        assert CLI_PATH is not None
        completed = subprocess.run(
            [CLI_PATH, "--json", *arguments],
            check=True,
            capture_output=True,
        )
        return json.loads(completed.stdout)


def _fake_ffmpeg(directory: Path) -> Path:
    if sys.platform == "win32":
        path = directory / "ffmpeg-ok.cmd"
        path.write_text(
            "@echo off\r\n"
            'if "%~1"=="-version" (\r\n'
            "  echo ffmpeg version python-fake-1\r\n"
            "  exit /b 0\r\n"
            ")\r\n"
            'set "last="\r\n'
            ":args\r\n"
            'if "%~1"=="" goto run\r\n'
            'set "last=%~1"\r\n'
            "shift\r\n"
            "goto args\r\n"
            ":run\r\n"
            '>"%last%" echo proxy-media\r\n'
            "exit /b 0\r\n",
            newline="",
        )
        return path
    path = directory / "ffmpeg-ok"
    path.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = "-version" ]; then echo "ffmpeg version python-fake-1"; exit 0; fi\n'
        "for last do :; done\n"
        'printf proxy-media > "$last"\n'
        "exit 0\n"
    )
    path.chmod(0o755)
    return path


if __name__ == "__main__":
    unittest.main()
