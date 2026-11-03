"""Validate and extract owned release tar inputs without following archive links.

Cargo interpretation stays in Cargo. This Python 3.8+ helper only handles archive members.
The first streaming pass validates the entire member graph before any writes;
the second repeats validation while creating exclusive regular files/directories.
It does not claim protection against concurrent filesystem namespace mutation.
"""

from __future__ import annotations

import gzip
import io
import os
from pathlib import Path
import shutil
import stat
import sys
import tarfile

MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_STREAM_BYTES = 1024 * 1024 * 1024
MAX_PAYLOAD_BYTES = 512 * 1024 * 1024
MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_MEMBERS = 100_000
MAX_PATH_BYTES = 4096
MAX_DEPTH = 128
COPY_BYTES = 64 * 1024
WINDOWS_DEVICES = {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"} | {
    f"{prefix}{number}" for prefix in ("COM", "LPT") for number in "123456789¹²³"
}


class ArchiveRefusal(ValueError):
    pass


class ReadBudget:
    def __init__(self, reader: io.BufferedIOBase) -> None:
        self.reader = reader
        self.consumed = 0

    def read(self, size: int) -> bytes:
        if size < 0:
            raise ArchiveRefusal("unbounded archive read requested")
        data = self.reader.read(min(size, MAX_STREAM_BYTES - self.consumed + 1))
        self.consumed += len(data)
        if self.consumed > MAX_STREAM_BYTES:
            raise ArchiveRefusal("expanded archive stream exceeds byte limit")
        return data


def member_parts(member: tarfile.TarInfo) -> tuple[str, ...]:
    name = member.name
    if len(name.encode("utf-8", errors="strict")) > MAX_PATH_BYTES:
        raise ArchiveRefusal("archive member path exceeds byte limit")
    if member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE, tarfile.DIRTYPE):
        raise ArchiveRefusal("archive links and special-file entries are refused")
    if member.linkname or member.sparse is not None:
        raise ArchiveRefusal("archive link/sparse metadata is refused")
    if member.mode & ~0o777:
        raise ArchiveRefusal("archive special permission bits are refused")
    if member.size < 0 or member.size > MAX_FILE_BYTES:
        raise ArchiveRefusal("archive member exceeds file byte limit")
    if member.isdir() and member.size:
        raise ArchiveRefusal("archive directory carries file data")
    if member.isdir():
        name = name[:-1] if name.endswith("/") else name
    parts = tuple(name.split("/"))
    if len(parts) > MAX_DEPTH:
        raise ArchiveRefusal("archive member path exceeds depth limit")
    for part in parts:
        if not part or part in (".", "..") or "\\" in part or ":" in part or "\0" in part:
            raise ArchiveRefusal("archive member path is not a confined relative path")
        if part.endswith((".", " ")) or part.split(".", 1)[0].upper() in WINDOWS_DEVICES:
            raise ArchiveRefusal("archive member has a Windows path/device alias")
    return parts


def checked_stream(raw: io.BufferedReader, package: bool):
    # gzip.GzipFile.read(size) bounds each decompressed read before tar sees it.
    inflated = gzip.GzipFile(fileobj=raw, mode="rb") if package else raw
    return tarfile.open(fileobj=ReadBudget(inflated), mode="r|"), inflated


def finish_stream(archive: tarfile.TarFile) -> None:
    # Validate padding and gzip completion as well as the visible tar members.
    while tail := archive.fileobj.read(COPY_BYTES):
        if any(tail):
            raise ArchiveRefusal("archive has non-padding data after its end marker")


def scan(raw: io.BufferedReader, package: bool, expected_root: str | None) -> tuple[dict, str | None]:
    archive, inflated = checked_stream(raw, package)
    entries: dict[tuple[str, ...], tuple[bool, int, int]] = {}
    payload = 0
    try:
        while (member := archive.next()) is not None:
            if len(entries) >= MAX_MEMBERS:
                raise ArchiveRefusal("archive member count exceeds limit")
            parts = member_parts(member)
            if parts in entries:
                raise ArchiveRefusal("archive repeats member")
            payload += member.size
            if payload > MAX_PAYLOAD_BYTES:
                raise ArchiveRefusal("archive total file bytes exceed limit")
            entries[parts] = (member.isdir(), member.size, member.mode)
            # Streaming next() does not need TarFile's optional member cache.
            archive.members.clear()
        finish_stream(archive)
        for parts in entries:
            for length in range(1, len(parts)):
                parent = entries.get(parts[:length])
                if parent is not None and not parent[0]:
                    raise ArchiveRefusal("archive file is also a parent directory")
        if package:
            roots = {parts[0] for parts in entries}
            if len(roots) != 1:
                raise ArchiveRefusal("archive must have exactly one package root")
            root = next(iter(roots))
            if expected_root is not None and root != expected_root:
                raise ArchiveRefusal("archive root does not match selected name/version")
            manifest = entries.get((root, "Cargo.toml"))
            if manifest is None or manifest[0]:
                raise ArchiveRefusal("archive has no exact root Cargo.toml")
            return entries, f"{root}/Cargo.toml"
        return entries, None
    finally:
        archive.close()
        if package:
            inflated.close()


def make_parents(destination: Path, parts: tuple[str, ...]) -> Path:
    path = destination
    for part in parts:
        path = path / part
        if path.is_symlink():
            raise ArchiveRefusal("extraction parent became a symlink")
        path.mkdir(exist_ok=True)
        if not path.is_dir():
            raise ArchiveRefusal("extraction parent is not a directory")
    return path


def process_archive(source: Path, destination: Path | None, *, package: bool,
                    expected_root: str | None = None) -> str | None:
    with source.open("rb") as raw:
        size = os.fstat(raw.fileno())
        if not stat.S_ISREG(size.st_mode) or size.st_size > MAX_ARCHIVE_BYTES:
            raise ArchiveRefusal("archive input is not a bounded regular file")
        entries, manifest = scan(raw, package, expected_root)
        if destination is None:
            return manifest
        if destination.is_symlink() or not destination.is_dir() or any(destination.iterdir()):
            raise ArchiveRefusal("extraction destination must be an owned empty directory")
        raw.seek(0)
        archive, inflated = checked_stream(raw, package)
        observed = set()
        directories = []
        try:
            while (member := archive.next()) is not None:
                parts = member_parts(member)
                identity = (member.isdir(), member.size, member.mode)
                if parts in observed or entries.get(parts) != identity:
                    raise ArchiveRefusal("archive changed between validation and extraction")
                observed.add(parts)
                parent = make_parents(destination, parts[:-1])
                target = parent / parts[-1]
                if member.isdir():
                    make_parents(destination, parts)
                    directories.append((target, member.mode))
                else:
                    data = archive.extractfile(member)
                    if data is None:
                        raise ArchiveRefusal("regular archive file has no byte stream")
                    with data, target.open("xb") as output:
                        shutil.copyfileobj(data, output, COPY_BYTES)
                    if target.stat().st_size != member.size:
                        raise ArchiveRefusal("extracted file length differs from archive")
                    os.chmod(target, member.mode)
                archive.members.clear()
            finish_stream(archive)
            if observed != entries.keys():
                raise ArchiveRefusal("archive member set changed during extraction")
            for target, mode in sorted(directories, key=lambda item: len(item[0].parts), reverse=True):
                os.chmod(target, mode)
        finally:
            archive.close()
            if package:
                inflated.close()
        return manifest


def main(argv: list[str]) -> int:
    if len(argv) == 3 and argv[1] == "inspect-package":
        manifest = process_archive(Path(argv[2]), None, package=True)
        assert manifest is not None
        sys.stdout.buffer.write(manifest.encode("utf-8") + b"\n")
    elif len(argv) == 5 and argv[1] == "extract-package":
        process_archive(Path(argv[2]), Path(argv[3]), package=True, expected_root=argv[4])
    elif len(argv) == 4 and argv[1] == "extract-source":
        process_archive(Path(argv[2]), Path(argv[3]), package=False)
    else:
        raise ArchiveRefusal("invalid private archive helper invocation")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except (ArchiveRefusal, OSError, tarfile.TarError, EOFError, UnicodeError) as error:
        print(f"archive validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
