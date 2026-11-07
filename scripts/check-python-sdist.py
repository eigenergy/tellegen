#!/usr/bin/env python3
"""Check source-package contents without extracting an untrusted archive."""
import sys
import tarfile
from pathlib import PurePosixPath


def check(path: str) -> None:
    with tarfile.open(path, "r:gz") as archive:
        names = []
        for member in archive.getmembers():
            name = PurePosixPath(member.name)
            if name.is_absolute() or ".." in name.parts or member.issym() or member.islnk():
                raise ValueError(f"unsafe archive member: {member.name}")
            relative = PurePosixPath(*name.parts[1:])
            if any(part in ("data", "fixtures") and relative.parts[i - 1] == "tests"
                   for i, part in enumerate(relative.parts) if i > 0):
                raise ValueError(f"test fixture leaked into source package: {relative}")
            names.append(str(relative))
        required = {"LICENSE", "pyproject.toml", "python/tellegen/_tellegen.pyi",
                    "python/tellegen/py.typed", "crates/tellegen-py/Cargo.toml"}
        missing = required - set(names)
        if missing:
            raise ValueError(f"source package is missing {sorted(missing)}")
        print(f"source package contains required files and excludes test fixtures ({len(names)} members)")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: check-python-sdist.py ARCHIVE.tar.gz")
    check(sys.argv[1])
