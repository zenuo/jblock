# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 09:05
**Active Feature:** feat-050 (done)

## Status

### What's Done

- [x] **feat-050** WASM analyze in Web Worker
  - `web/src/analyze.worker.ts` owns `init` / `analyzeDump` / `analyzeDumps`
  - `web/src/analyzer.ts` main-thread RPC via `new Worker(..., { type: "module" })`
  - Main thread no longer imports `./wasm/jblock`
  - `App.onFiles` sets `busyPhase` + `requestAnimationFrame` before `file.text()`
  - e2e `FEATURE_CHECKS["feat-050"]`; feat-004 updated for worker wire

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: transfer dump as UTF-8 `ArrayBuffer` to cut structured-clone cost
2. Optional: virtualize Results for huge thread tables

## Decisions Made

- Keep `preloadWasm` / `isWasmReady` / `analyze` / `analyzeMany` API stable for App
- Early loading overlay is part of feat-050 so file read stall is no longer “silent”
- Discriminated request body type used instead of `Omit<union, "id">` (TS limitation)

## Evidence of Completion

### Verification

```text
$ ./init.sh
=== cargo test === … 93 passed
=== pnpm lint / typecheck / build === ok (analyze.worker-*.js in dist)
=== e2e feature matrix === Summary: 50/50 features PASS
```
