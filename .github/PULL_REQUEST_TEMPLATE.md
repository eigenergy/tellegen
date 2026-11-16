<!-- What changes, and why. Link the issue it closes with "Closes #N". -->

## Checklist

- [ ] `just ci` passes locally (or the relevant subset, named below)
- [ ] a `patch` changeset is added if anything under `packages/` changed (`npm run changeset`); none is needed for `apps/web`, `examples/`, or the crates
- [ ] nothing breaks existing code, and no commit or title is marked breaking (`!:`)
- [ ] it reuses the existing helpers and components instead of adding a second one
- [ ] anything experimental is off by default
- [ ] docs under `docs/src/` are updated for any user-visible change
- [ ] the title reads well as a changelog entry (the crate's changelog lists pull requests by title; `feat:` or `fix:` files it under Added or Fixed)

This repository merges with merge commits only. See [CONTRIBUTING.md](../CONTRIBUTING.md).
