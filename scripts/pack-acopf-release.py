#!/usr/bin/env python3
"""Validate and package the separate AC OPF reactor for a tagged release."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tomllib

FILES = (
    "tellegen_acopf_wasi.wasm",
    "POUNCE-EPL-2.0.txt",
    "POUNCE-NOTICE.txt",
    "POUNCE-SOURCE.json",
)


def package(source: Path, output: Path, tag: str, revision: str):
    if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        raise ValueError("expected a stable crate release tag")
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("expected a full source revision")
    version = tomllib.loads((source / "crates/tellegen/Cargo.toml").read_text())["package"]["version"]
    if tag != f"v{version}":
        raise ValueError("tag does not match the source crate version")
    assets = source / "target/experimental-acopf"
    manifest = json.loads((assets / "POUNCE-SOURCE.json").read_text())
    wasm = (assets / FILES[0]).read_bytes()
    if not wasm.startswith(b"\0asm\x01\0\0\0"):
        raise ValueError("asset is not a WebAssembly module")
    if manifest.get("development_only") is not False or manifest.get("tellegen_worktree_dirty"):
        raise ValueError("only a clean release artifact may be distributed")
    if manifest.get("tellegen_source") != f"https://github.com/eigenergy/tellegen/tree/{revision}":
        raise ValueError("manifest names a different source revision")
    if (manifest.get("artifact_sha256") != hashlib.sha256(wasm).hexdigest()
            or manifest.get("artifact_bytes") != len(wasm)):
        raise ValueError("manifest does not match the reactor")
    if manifest.get("license") != "EPL-2.0" or not manifest.get("crates"):
        raise ValueError("license and corresponding source are required")
    for name in FILES[1:3]:
        if (assets / name).read_bytes() != (source / "licenses" / name).read_bytes():
            raise ValueError(f"{name} does not match the released source")
    manifest.update({
        "abi": "tellegen-acopf-wasi/1",
        "release_tag": tag,
        "engine_version": json.loads((source / "packages/engine/package.json").read_text())["version"],
        "tellegen_source_archive": f"https://github.com/eigenergy/tellegen/archive/{revision}.tar.gz",
    })
    output.mkdir(parents=True, exist_ok=True)
    for name in FILES[:-1]:
        shutil.copyfile(assets / name, output / name)
    (output / FILES[-1]).write_text(json.dumps(manifest, indent=2) + "\n")
    sums = [f"{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n" for name in FILES]
    (output / "ACOPF-SHA256SUMS.txt").write_text("".join(sums))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    package(args.source, args.output, args.tag, args.revision)
