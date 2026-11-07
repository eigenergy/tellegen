---
"@tellegen/engine": minor
"@tellegen/svelte": minor
"@tellegen/webmcp": minor
---

Add a common Settings panel and a Rust-owned calculation settings contract across TypeScript, WebMCP, native, CLI and HTTP interfaces. Applying settings stages the next explicit solve without replacing the accepted result. Retained balanced Studies keep their effective options and active constraint selections. Saved PowerIO modules record the calculation and only the options that differ from the engine defaults; reopening a file written by another engine version drops options this engine does not know and logs a warning instead of refusing the file. The generated settings types name calculations with the `Calculation` union. Browser display preferences persist separately, and API mutations check case and settings revisions. `tellegen solve-mc` now resolves its options through the same contract, so an unknown option key is an error rather than being silently ignored.
