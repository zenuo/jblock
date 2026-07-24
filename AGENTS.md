# jblock

Single-page Java thread dump analyzer. Rust parser compiled to WebAssembly,
consumed by a React + Vite frontend. See `README.md` for the full command list
and architecture.

## Cursor Cloud specific instructions

### Layout & services

- Rust WASM crate at repo root (`Cargo.toml`, `src/lib.rs`, `src/parser.rs`).
- Frontend lives in `web/` (React + Vite + TypeScript). All `pnpm` commands run
  from `web/`.
- There is a single dev service: the Vite dev server (`web`, port `5173`).

### Non-obvious gotchas

- The parsing logic is pure Rust in `src/parser.rs` (no `wasm-bindgen`) so it is
  unit-testable with `cargo test` on the host target; `src/lib.rs` only holds the
  wasm bindings. Add parser tests in `parser.rs`.
- `web/src/wasm/` is a **generated** wasm-pack artifact (gitignored). It must
  exist before `vite`/`tsc` can resolve imports. Run `pnpm run wasm` (or
  `pnpm run dev`, which builds it first) to (re)generate it.
- WASM is **not** hot-reloaded. After editing any `src/*.rs`, re-run
  `pnpm run wasm` (or restart `pnpm run dev`); Vite HMR only tracks the JS/TS.
- The WASM is imported via `wasm-pack --target web`; Vite resolves the
  `.wasm` asset through `new URL(..., import.meta.url)`, so no extra Vite wasm
  plugin is needed.
- pnpm blocks `esbuild`'s install script by default; it is allow-listed via
  `pnpm.onlyBuiltDependencies` in `web/package.json` (avoid the interactive
  `pnpm approve-builds`).
- Keep `web/src/types.ts` in sync with the Rust `Analysis` struct in
  `src/parser.rs` (serde serializes enums as kebab-case, e.g. `thread-mx-bean`).

### Verify quickly

- Parser: `cargo test`
- Frontend: from `web/` run `pnpm run typecheck`, `pnpm run lint`,
  `pnpm run build`, and `pnpm run dev` (then load the "Load sample" button).
