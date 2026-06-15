# Documentation

Start with the README at the repository root. It gives the demo behavior,
development commands, API surface, and deployment path.

This directory holds the longer notes that should not crowd the first page.

## Release Notes

| file | use it for |
|---|---|
| [tamu-coordinates.md](tamu-coordinates.md) | Data provenance, staged file names, and coordinate caveats for the served ACTIVSg cases. |
| [synthetic-layout.md](synthetic-layout.md) | The fallback layout used when staged TAMU coordinates are absent. |
| [display-format.md](display-format.md) | PowerWorld `.pwd` display parsing, coordinate projection, and the future display data boundary. |
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

Dropped case files are parsed in the browser. The server does not receive them.
