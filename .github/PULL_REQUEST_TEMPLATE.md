<!-- What changes, and why. Link the issue it closes with "Closes #N". -->

## Checklist

- [ ] `just ci` passes locally (or the relevant subset, named below)
- [ ] a changeset is added if `packages/engine`, `packages/svelte`, or `packages/webmcp` changed (`npm run changeset`); none is needed for `apps/web`, `examples/`, or the crates
- [ ] docs under `docs/src/` are updated for any user-visible change
- [ ] commit messages read well in a changelog (release-plz writes the crate's from them)

This repository merges with merge commits only. See [CONTRIBUTING.md](../CONTRIBUTING.md).
