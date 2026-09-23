# Security

## Reporting a vulnerability

Report vulnerabilities privately through GitHub's
[private vulnerability reporting](https://github.com/eigenergy/tellegen/security/advisories/new)
for this repository. Do not open a public issue for a security problem.

You should hear back within a week. Reports are handled by the maintainers,
and a fix is released through the normal crate and package pipeline with a
GitHub security advisory once it is available.

## Supported versions

Only the latest published release of each surface receives fixes:

| Surface | Where |
| --- | --- |
| `tellegen` crate | crates.io |
| `@tellegen/engine`, `@tellegen/svelte`, `@tellegen/webmcp` | npm |
| the hosted demo | tellegen.dev, deployed from `main` |

## Scope

The browser packages parse untrusted case files on the user's device and never
upload them; a parser or solver crash from a malicious case file is in scope.
The HTTP server is a static file host with a solve API and is documented in
[docs/src/http-api.md](docs/src/http-api.md); its deployment and privacy
posture are in [docs/src/deployment.md](docs/src/deployment.md) and
[docs/src/privacy.md](docs/src/privacy.md).
