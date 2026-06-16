# Documentation

Start with the README at the repository root. It gives the demo behavior,
development commands, API surface, and deployment path.

This directory holds the longer notes that should not crowd the first page.

## Release Notes

| file | use it for |
|---|---|
| [tamu-coordinates.md](tamu-coordinates.md) | Data provenance, staged file names, and coordinate caveats for the served ACTIVSg cases. |
| [synthetic-layout.md](synthetic-layout.md) | Synthetic topology layouts for explicit fallback data and local placement. |
| [geo-sidecars.md](geo-sidecars.md) | Browser-only coordinate sidecars for local case files. |
| [display-format.md](display-format.md) | PowerWorld `.pwd` display parsing, coordinate projection, and the future display data boundary. |
| [../frontend/src/routes/privacy/+page.svelte](../frontend/src/routes/privacy/+page.svelte) | Public privacy page for local parsing and future opt-in sharing. |
| [../deploy/DEPLOY.md](../deploy/DEPLOY.md) | Host setup, GitHub Actions deploy, health checks, and public hardening. |

## Architecture Notes

| file | use it for |
|---|---|
| [direction.md](direction.md) | The Rust, Svelte, Julia, powerio, and PowerDiff.jl boundary. |
| [research-notes.md](research-notes.md) | Point in time ecosystem checks behind the direction note. |

## Public Caveats

The served networks are TAMU ACTIVSg synthetic grids. Their coordinates describe
fictional cases on geographic footprints, not surveyed infrastructure. tellegen
labels fallback coordinates as synthetic and labels `.pwd` positions as
approximate because those positions come from a diagram projection.

Dropped case files and coordinate sidecars are parsed in the browser. The server
does not receive them. Files without complete coordinates can use uploaded
sidecar coordinates or a synthetic topology layout placed by the user.
