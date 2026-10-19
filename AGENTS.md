# Working in FujiMod

Use TypeScript for all maintained browser, worker, Node, build and test source
(`.ts`, `.mts`, `.cts`, `.tsx`). Do not add JavaScript source files or executable
inline HTML scripts. Generate browser/wasm-bindgen JavaScript only in ignored
build directories. Keep types in the implementation rather than handwritten
`.d.ts` facades for untyped code.

Every TypeScript configuration must explicitly disable `allowJs`, including
inherited effective settings. Never pass `--allowJs` to tooling. Check source,
scripts and tests without `@ts-nocheck`, `@ts-ignore`, blanket `any` or weakened
compiler checks. Run `npm run check:typescript-policy` and the affected type
checks and tests before committing. The policy check runs in CI.
