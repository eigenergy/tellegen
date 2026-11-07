"""Build the validation runner against an explicitly selected Tellegen checkout."""

import argparse
import json
import shutil
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tellegen-root', required=True, type=Path)
    parser.add_argument('--build-dir', required=True, type=Path)
    parser.add_argument('--target-dir', required=True, type=Path)
    args = parser.parse_args()
    args.build_dir.mkdir(parents=True, exist_ok=False)
    (args.build_dir / 'src').mkdir()
    here = Path(__file__).resolve().parent
    dependency = json.dumps(str(args.tellegen_root.resolve() / 'crates/tellegen'))
    (args.build_dir / 'Cargo.toml').write_text(
        '[package]\nname = "texas7k-energized-runner"\nversion = "0.1.0"\nedition = "2021"\n'
        f'[dependencies]\ntellegen = {{path = {dependency}}}\n'
        'powerio = {version = "=0.11.7", default-features = false}\nserde_json = "1"\n')
    shutil.copyfile(here / 'energized-runner.rs', args.build_dir / 'src/main.rs')
    shutil.copyfile(here / 'runner-Cargo.lock', args.build_dir / 'Cargo.lock')
    subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path',
                    str(args.build_dir / 'Cargo.toml'), '--target-dir', str(args.target_dir)], check=True)


if __name__ == '__main__':
    main()
