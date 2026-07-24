# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 09:25
**Active Feature:** feat-011 (done)

## Status

### What's Done

- [x] feat-001 … feat-010
- [x] feat-011 Move Java codegen to frontend (shrink WASM)

### What's In Progress

- [ ] None

### What's Next

1. Optional further WASM size work / new features.

## Blockers / Risks

- [ ] Risk: `pnpm -C web run build` requires `wasm-pack` + `wasm32-unknown-unknown`.

## Decisions Made

- **feat-011**: Page generation in `web/src/codegen.ts`. Rust `src/codegen.rs` is `cfg(not(target_arch = "wasm32"))` so host `cargo test` and `examples/gen_java` still work; wasm-bindgen no longer exports `generateJava`.

## Evidence of Completion

- [x] `cargo test` 16 passed
- [x] WASM `jblock_bg.wasm` 1,049,881 → 1,044,453 (−5,428 bytes); no `generateJava` in wasm pkg
- [x] `./init.sh` (pending run)

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
