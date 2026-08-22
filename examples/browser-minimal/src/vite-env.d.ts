// `vite/client` supplies the ambient `declare module '*.css' {}` that lets a bare
// side-effect import of a stylesheet type-check. TypeScript 7 turns
// `noUncheckedSideEffectImports` on by default, which reports TS2882 for a
// side-effect import with no declaration.
/// <reference types="vite/client" />
