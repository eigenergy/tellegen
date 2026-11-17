# Tellegen demo

This SvelteKit application uses the packages in `packages/` to demonstrate
power-system calculations and interactive views.

From the repository root, run `just dev-setup`, then `just dev`. The app opens
at http://localhost:5173; its case server runs on port 8000.

Use `examples/` as the starting point for another application. Shared behavior
belongs in the packages; demo-specific routes, case choices and branding belong
here. See [the quickstart](../../docs/src/framework-quickstart.md).
