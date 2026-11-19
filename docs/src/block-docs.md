# Block diagrams from the code

RMS dynamics documents its structure in the code itself, from the engine
(`crates/tellegen/src/rms`) to the app (`packages/svelte`): the code is
divided into **blocks**, and each block carries its own explanation.
`tellegen-blocks` reads them and draws an interactive block diagram: the
blocks of one level and what uses what, a summary on hover, the sub-blocks on
a click, and the full documentation and code beside it. Nobody has to read the
code to see how a run goes through the engine and the app, and the diagram
cannot drift from the code, because the code is all it reads.

- **On a running server:** `/docs/blocks/rms` (drawn when `tellegen-server` was
  built, so it shows exactly the code that server runs).
- **Locally:** `just blocks` writes `target/blocks/rms.html`.

Protection relays (`crates/tellegen/src/protection`) are documented the same
way: `/docs/blocks/protection` on a running server, and
`target/blocks/protection.html` from `just blocks`. Their link to RMS runs
(`rms::protect`) and the app's protection views are blocks of the RMS diagram.

## Declaring a block in Rust

One tag in the documentation the code already has. There are three kinds,
nested by where they are:

```rust,ignore
//! Time integration of `M ẋ = f(x)` with the implicit trapezoidal rule, …
//!
//! @block Time integration                 <- a module: the whole file

/// Advance `(x, f)` by one step `h` …
// @block One step                          <- an item: this fn, struct, enum,
pub(crate) fn step(…) {                     //    trait, impl, type, const, mod
    // @block Predict                       <- a section of code, up to the
    // Newton starts from the parabola …    //    next section at this depth,
    …                                       //    `// @end`, or the closing brace
    // @block Newton iterations
    // Up to 25 corrections …
    …
}
```

- **Module:** `@block <title>` in the file's `//!` docs. The documentation is
  the module docs; the tag closes them.
- **Item:** a `// @block <title>` line right under an item's `///` docs. The
  documentation is the item's docs. The tag is an ordinary comment, so
  rustdoc and the JSON schemas derived from doc comments never show it.
- **Section:** an own-line `// @block <title>` comment, followed by the `//`
  lines that document it. A section can group anything: statements in a
  function, methods in an `impl`, items or `mod` declarations in a module
  (`rms/mod.rs` groups its modules into stages this way). `// @end` ends one
  early.

The first paragraph is the summary the diagram shows in a box; the rest
appears beside it.

## Declaring a block in TypeScript and Svelte

The tag is a JSDoc tag at the end of the docs:

```ts
/** Pure helpers of the live workbench, … (the file's first JSDoc)
 * @block Live workbench logic */             <- a module: the whole file

/** The simulated time the live view's pacing loop aims for …
 * @block Pacing target */                    <- an item: a function, class,
export function paceTarget(…) { … }           //    method, type or constant
```

- **Module:** `@block <title>` in the first `/** … */` of the file, or of a
  Svelte component's first `<script>`. Each file is a module named after
  it; the folders are groups (`State and logic`, `Components`).
- **Item:** `@block <title>` in the JSDoc of a function, class, method
  (`#private` ones too), interface, type or constant.
- **Section:** `// @block <title>` with its `//` docs, as in Rust.

A Svelte component's script and the components and expressions of its
markup count for its arrows; other JSDoc tags (`@param`) stay in the text.

## What comes from the code

| | From |
|---|---|
| Id | The path: `rms::integrate::Stepper::step`, `app::logic::live::LiveView::run`, a section `…#newton-iterations` |
| Parent | The smallest block around it; a Rust module's parent is the block around its `mod` declaration, a file's its folder |
| Order | Where it is: sections in a function are numbered steps |
| Arrows | A name used in one block and defined in another: a type, function, method, constant, module or component, within one language. Code under `#[cfg(test)]`, `use` and `import` declarations do not count |
| Size, location | The block's lines in its file |

Nothing is written twice: there is no diagram to keep in step with the code,
no list of edges, no separate documentation. The engine and the app meet in
the wasm engine package, which the diagram does not draw as arrows.

## Checks

`cargo test -p tellegen-blocks` (part of `cargo test --workspace`) fails when
the annotations are incomplete, naming the file and line: a module without a
block, a block without documentation, an unknown tag in Rust docs, an `@end`
without a section, or a block tag that documents no item. The server's build
script reports the same as build warnings.

## Command line

```sh
cargo run -p tellegen-blocks -- --preset rms --out target/blocks/rms.html
cargo run -p tellegen-blocks -- --preset protection --out target/blocks/protection.html
cargo run -p tellegen-blocks -- crates/tellegen/src/rms \
    --title "tellegen RMS engine" --out target/blocks/rms.html \
    [--json target/blocks/rms.json] [--source-url <prefix>] [--check]
```

`--preset rms` draws the engine and the app's dynamics code together,
`--preset protection` the relay engine;
`--source-url` links each block to its lines (for example
`https://github.com/<org>/<repo>/blob/<rev>/`); `--check` exits with an error
when there are annotation issues. The generator has no dependencies and works
on any Rust module tree or TypeScript folder, so another part of the code can
adopt it by adding its tags and one command.
