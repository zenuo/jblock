# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 10:15
**Active Feature:** feat-021 (done)

## Status

### What's Done

- [x] feat-001 … feat-020 (on main)
- [x] feat-021 WASM preload + analyze loading UI

### What's In Progress

- [ ] None on this branch

### What's Next

1. Enable GitHub Pages source = GitHub Actions in repo settings if not already.

## Decisions Made

- Preload WASM via `preloadWasm()` on App mount (same singleton as `ensureReady`).
- Busy UI uses a fixed overlay with two phases: `wasm` ("Loading analyzer…") and `analyzing` ("Analyzing dump…").
- Yield with `setTimeout(0)` before `analyzeDump` so the analyzing overlay can paint (WASM parse is sync on the main thread).

## Evidence of Completion

- [x] `preloadWasm` / `isWasmReady` in `web/src/analyzer.ts`
- [x] Overlay in `App.tsx` (`data-testid="loading-overlay"`)
- [x] `./init.sh` green

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
