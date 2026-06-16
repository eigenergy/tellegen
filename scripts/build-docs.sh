#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

cd "$repo_root"
mdbook build
mkdir -p docs/book/assets
cp docs/assets/hero.svg docs/book/assets/hero.svg
touch docs/book/.nojekyll
