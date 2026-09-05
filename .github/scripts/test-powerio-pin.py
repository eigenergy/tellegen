#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import tempfile
from pathlib import Path


SCRIPT = Path(__file__).with_name("powerio-pin.py")
SPEC = importlib.util.spec_from_file_location("powerio_pin", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
pin = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pin)

OLD = "1" * 40
NEW = "2" * 40


def manifest(revisions: dict[str, str] | None = None) -> str:
    revisions = revisions or {}
    lines = ["[patch.crates-io]"]
    for crate in pin.POWERIO_CRATES:
        revision = revisions.get(crate, OLD)
        lines.append(
            f'{crate} = {{ git = "{pin.POWERIO_REPOSITORY}", rev = "{revision}" }}'
        )
    return "\n".join(lines) + "\n"


def lock(revision: str = OLD) -> str:
    source = f"git+{pin.POWERIO_REPOSITORY}?rev={revision}#{revision}"
    return "\n".join(
        f'[[package]]\nname = "{crate}"\nversion = "0.11.0"\nsource = "{source}"\n'
        for crate in pin.POWERIO_CRATES
    )


def registry_lock() -> str:
    return lock().replace(
        f"git+{pin.POWERIO_REPOSITORY}?rev={OLD}#{OLD}",
        "registry+https://github.com/rust-lang/crates.io-index",
    ).replace('version = "0.11.0"', 'version = "0.11.0"\nchecksum = "' + "a" * 64 + '"')


def rejects(callable_) -> None:
    try:
        callable_()
    except SystemExit:
        return
    raise AssertionError("invalid PowerIO pins were accepted")


with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    manifest_path.write_text(manifest())
    lock_path.write_text(lock())

    revision = pin.manifest_revision(manifest_path)
    assert revision == OLD
    pin.check_lock(lock_path, revision)

    manifest_path.write_text(manifest({"powerio-core": NEW}))
    rejects(lambda: pin.manifest_revision(manifest_path))

    manifest_path.write_text(manifest())
    lock_path.write_text(lock(NEW))
    rejects(lambda: pin.check_lock(lock_path, OLD))

    pin.set_manifest_revision(manifest_path, NEW)
    assert pin.manifest_revision(manifest_path) == NEW
    assert manifest_path.read_text().count(NEW) == len(pin.POWERIO_CRATES)
    rejects(lambda: pin.set_manifest_revision(manifest_path, "main"))

    # Publication removes the patches. The registry lock must cover all six
    # components with one version and the checksums Cargo records.
    manifest_path.write_text('[workspace.dependencies]\npowerio = "0.11"\n')
    assert pin.manifest_revision(manifest_path) is None
    rejects(lambda: pin.check_lock(lock_path, None))
    lock_path.write_text(registry_lock())
    pin.check_lock(lock_path, None)

    for invalid in [
        registry_lock().replace('checksum = "' + "a" * 64 + '"', '', 1),
        registry_lock().replace('version = "0.11.0"', 'version = "0.10.0"', 1),
        registry_lock().replace('registry+https://github.com/rust-lang/crates.io-index',
                                'registry+https://example.invalid/index', 1),
        registry_lock() + lock(),
    ]:
        lock_path.write_text(invalid)
        rejects(lambda: pin.check_lock(lock_path, None))
    lock_path.write_text(registry_lock())
    rejects(lambda: pin.check_lock(lock_path, NEW))

    # A future candidate can start from the published baseline, including a
    # manifest that already patches an unrelated crate.
    for published in [
        '[workspace.dependencies]\npowerio = "0.11"\n',
        '[patch.crates-io]\nother = { path = "other" }\n\n[profile.release]\nlto = true\n',
    ]:
        manifest_path.write_text(published)
        pin.set_manifest_revision(manifest_path, NEW)
        assert pin.manifest_revision(manifest_path) == NEW
        assert manifest_path.read_text().count(NEW) == len(pin.POWERIO_CRATES)
        if "other" in published:
            parsed = pin.read_toml(manifest_path)
            assert parsed["patch"]["crates-io"]["other"] == {"path": "other"}
            assert parsed["profile"]["release"]["lto"] is True

    partial = manifest().replace(
        f'powerio-core = {{ git = "{pin.POWERIO_REPOSITORY}", rev = "{OLD}" }}\n', ''
    )
    manifest_path.write_text(partial)
    rejects(lambda: pin.set_manifest_revision(manifest_path, NEW))
    assert manifest_path.read_text() == partial

print("PowerIO pin tests passed")
