# Tellegen

Tellegen is a framework for power-system applications. It provides a Rust
numerical engine, browser packages, and Svelte components. The hosted
[demo](https://tellegen.dev) is one application built with those packages.

Use the demo to open a case, choose a calculation, inspect results, and change
loads or equipment settings. Supported calculations depend on the imported
model. A parsed file is not necessarily ready to solve; the app explains
unsupported models and missing data.

## Start here

- [Run the demo locally](getting-started.md).
- [Build an application](framework-quickstart.md) with the viewer or engine.
- [Load your files](local-case-files.md), including [ZIPs and folders](collection-import.md).
- [Choose a calculation](formulations.md) and read its assumptions.
- [Connect an agent](webmcp.md) to the browser, or use the [Python MCP server](mcp.md).

Browser calculations run in WebAssembly. Local case files stay in the browser
unless the host application explicitly sends them elsewhere. The demo server
provides bundled cases; applications using local files do not need it.

Sensitivity previews estimate nearby changes. Committing an edit runs the
calculation again. A preview is not a replacement for checking the new solution.

For package APIs, see the [engine reference](engine-api.md),
[Svelte components](frontend-package.md), [Python](python.md), and
[HTTP server](http-api.md). The [architecture](architecture.md) explains which
parts belong to the framework and which belong to the demo.
