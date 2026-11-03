#!/usr/bin/env python3
"""Execute the real bounded archive helper against small positive/negative inputs."""

from __future__ import annotations

import gzip
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "xtask/src/local_release_archive.py"
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("local_release_archive", HELPER)
assert SPEC and SPEC.loader
ARCHIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ARCHIVE)


def regular(name: str, data: bytes = b"bytes", mode: int = 0o644):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    return info, data


def typed(name: str, kind: bytes, link: str = ""):
    info = tarfile.TarInfo(name)
    info.type = kind
    info.linkname = link
    info.mode = 0o755
    return info, b""


def write_tar(path: Path, members, package: bool = True, suffix: bytes = b""):
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for info, data in members:
            archive.addfile(info, io.BytesIO(data))
    content = raw.getvalue() + suffix
    path.write_bytes(gzip.compress(content) if package else content)


class ArchiveControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="adze archive λ with spaces-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "input.crate"
        self.destination = self.root / "extract"
        self.destination.mkdir()

    def refuse(self, members, message: str = "", package: bool = True, suffix: bytes = b""):
        # A valid member comes first: every later refusal must still happen
        # before any destination member is written.
        write_tar(self.source, [regular("pkg-1/Cargo.toml"), *members], package, suffix)
        with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, message):
            ARCHIVE.process_archive(self.source, self.destination, package=package,
                                    expected_root="pkg-1" if package else None)
        self.assertEqual([], list(self.destination.iterdir()))

    def test_positive_binary_unicode_package_and_source_exports(self):
        payload = bytes([0, 255, 128, 13, 10, 195, 169, 0, 254])
        members = [typed("pkg-1", tarfile.DIRTYPE), regular("pkg-1/Cargo.toml"),
                   regular("pkg-1/λ source.bin", payload, 0o755)]
        write_tar(self.source, members)
        self.assertEqual("pkg-1/Cargo.toml", ARCHIVE.process_archive(self.source, None, package=True))
        ARCHIVE.process_archive(self.source, self.destination, package=True, expected_root="pkg-1")
        self.assertEqual(payload, (self.destination / "pkg-1/λ source.bin").read_bytes())
        if os.name == "posix":
            self.assertEqual(0o755, (self.destination / "pkg-1/λ source.bin").stat().st_mode & 0o777)
        source_dest = self.root / "source"
        source_dest.mkdir()
        write_tar(self.source, [regular("Cargo.toml"), regular("src/lib.rs", b"source")], False)
        ARCHIVE.process_archive(self.source, source_dest, package=False)
        self.assertEqual(b"source", (source_dest / "src/lib.rs").read_bytes())

    def test_absolute_parent_backslash_drive_device_and_alias_names_refused(self):
        cases = ["/escape", "../escape", "pkg-1/../../escape", "pkg-1/./escape",
                 "pkg-1//escape", "pkg-1/escape\\child", "C:/escape", "pkg-1/C:escape",
                 "pkg-1/NUL", "pkg-1/aux.rs", "pkg-1/COM¹.txt", "pkg-1/.. ", "pkg-1/name."]
        for name in cases:
            with self.subTest(name=name):
                self.refuse([regular(name)])
                self.assertFalse((self.root / "escape").exists())

    def test_links_devices_fifo_sparse_and_unknown_types_refused_before_writes(self):
        cases = [(tarfile.SYMTYPE, "../../escape"), (tarfile.LNKTYPE, "../../escape"),
                 (tarfile.CHRTYPE, ""), (tarfile.BLKTYPE, ""), (tarfile.FIFOTYPE, ""),
                 (tarfile.GNUTYPE_SPARSE, ""), (b"Z", "")]
        for kind, link in cases:
            with self.subTest(kind=kind):
                self.refuse([typed("pkg-1/escape", kind, link)])
        disguised = regular("pkg-1/escape")
        disguised[0].linkname = "../../escape"
        self.refuse([disguised], "link/sparse")

    def test_duplicate_file_parent_collision_multiple_root_and_missing_manifest_refused(self):
        for members in [
            [regular("pkg-1/Cargo.toml")],
            [regular("pkg-1/data"), regular("pkg-1/data/child")],
            [regular("other/Cargo.toml")],
        ]:
            with self.subTest(members=[info.name for info, _ in members]):
                self.refuse(members)
        write_tar(self.source, [regular("pkg-1/nested/Cargo.toml")])
        with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, "exact root Cargo.toml"):
            ARCHIVE.process_archive(self.source, self.destination, package=True)
        self.assertEqual([], list(self.destination.iterdir()))

    def test_special_permissions_and_directory_payload_refused(self):
        self.refuse([regular("pkg-1/setuid", mode=0o4755)], "permission")
        info, _ = typed("pkg-1/directory", tarfile.DIRTYPE)
        info.size = 1
        self.refuse([(info, b"x")], "directory carries")

    def test_member_path_depth_and_total_file_limits_are_enforced(self):
        cases = [
            ("MAX_MEMBERS", 1, [regular("pkg-1/second")], "member count"),
            ("MAX_PATH_BYTES", 17, [regular("pkg-1/path-too-long")], "path exceeds byte"),
            ("MAX_DEPTH", 2, [regular("pkg-1/deep/file")], "depth"),
            ("MAX_FILE_BYTES", 5, [regular("pkg-1/large", b"123456")], "file byte"),
            ("MAX_PAYLOAD_BYTES", 5, [regular("pkg-1/extra", b"x")], "total file bytes"),
        ]
        for field, value, members, message in cases:
            with self.subTest(field=field), patch.object(ARCHIVE, field, value):
                self.refuse(members, message)

    def test_raw_and_expanded_stream_limits_are_enforced(self):
        write_tar(self.source, [regular("pkg-1/Cargo.toml"), regular("pkg-1/data", b"x" * 1024)])
        with patch.object(ARCHIVE, "MAX_ARCHIVE_BYTES", self.source.stat().st_size - 1):
            with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, "bounded regular file"):
                ARCHIVE.process_archive(self.source, self.destination, package=True)
        with patch.object(ARCHIVE, "MAX_STREAM_BYTES", 512):
            with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, "expanded archive stream"):
                ARCHIVE.process_archive(self.source, self.destination, package=True)
        self.assertEqual([], list(self.destination.iterdir()))

    def test_pax_effective_path_and_nonpadding_trailer_are_validated(self):
        info, data = regular("pkg-1/safe")
        info.pax_headers = {"path": "../escape"}
        self.refuse([(info, data)], "confined")
        self.refuse([], "non-padding", suffix=b"second hidden archive")
        write_tar(self.source, [regular("pkg-1/Cargo.toml")])
        self.source.write_bytes(self.source.read_bytes()[:-4])
        with self.assertRaises((EOFError, tarfile.TarError, OSError)):
            ARCHIVE.process_archive(self.source, self.destination, package=True)
        self.assertEqual([], list(self.destination.iterdir()))

    def test_destination_is_exclusive_and_selected_root_is_exact(self):
        write_tar(self.source, [regular("pkg-1/Cargo.toml")])
        with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, "selected name/version"):
            ARCHIVE.process_archive(self.source, self.destination, package=True, expected_root="pkg-2")
        sentinel = self.destination / "sentinel"
        sentinel.write_bytes(b"existing owned bytes")
        with self.assertRaisesRegex(ARCHIVE.ArchiveRefusal, "owned empty directory"):
            ARCHIVE.process_archive(self.source, self.destination, package=True)
        self.assertEqual(b"existing owned bytes", sentinel.read_bytes())

    def test_actual_cli_nonzero_refusal_and_success(self):
        write_tar(self.source, [regular("pkg-1/Cargo.toml"), typed("pkg-1/link", tarfile.SYMTYPE, "../../escape")])
        rejected = subprocess.run([sys.executable, str(HELPER), "extract-package", str(self.source),
                                   str(self.destination), "pkg-1"], capture_output=True, timeout=10)
        self.assertEqual(1, rejected.returncode)
        self.assertIn(b"archive validation failed:", rejected.stderr)
        self.assertEqual(b"", rejected.stdout)
        self.assertEqual([], list(self.destination.iterdir()))
        write_tar(self.source, [regular("pkg-1/Cargo.toml")])
        inspected = subprocess.run([sys.executable, str(HELPER), "inspect-package", str(self.source)],
                                   capture_output=True, timeout=10)
        self.assertEqual(0, inspected.returncode, inspected.stderr)
        self.assertEqual(b"pkg-1/Cargo.toml\n", inspected.stdout)


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ArchiveControls)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    # This required class must execute all ten methods, with no skipped control.
    raise SystemExit(0 if result.wasSuccessful() and result.testsRun == 10 and not result.skipped else 1)
