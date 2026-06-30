# Browser Minimal Example

This example is the first integration reference for `@tellegen/engine`. It uses Vite and plain TypeScript, not the hosted demo app.

It shows the downstream app flow:

1. import `@tellegen/engine`;
2. parse a MATPOWER case in browser WebAssembly;
3. create a `Study`;
4. preview a demand edit;
5. commit the edit with a sensitivity request; and
6. render the result.

Run it from the repository root:

```sh
npm --prefix packages/engine run wasm
npm --prefix packages/engine run build
npm --prefix examples/browser-minimal install
npm --prefix examples/browser-minimal run dev
```
