// `vite/client` supplies the ambient `declare module '*.css' {}` that lets a bare
// side-effect import of a stylesheet type-check. TypeScript 7 turns
// `noUncheckedSideEffectImports` on by default, which reports TS2882 for a
// side-effect import with no declaration, so this reference is load-bearing for
// `import 'maplibre-gl/dist/maplibre-gl.css'` in TellegenMap.svelte.
//
// It must stay out of `src/lib`: svelte-package publishes `src/lib` only, and a
// `vite/client` reference in the published types would force every consumer of
// this package to install Vite just to type-check.
/// <reference types="vite/client" />
