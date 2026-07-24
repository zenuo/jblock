# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 09:50
**Active Feature:** feat-013..019 (done)

## Status

### What's Done

- [x] feat-001 … feat-012
- [x] feat-013 Problem-first findings summary
- [x] feat-014 Aggregate lock contention by lock
- [x] feat-015 Thread table filter and sort
- [x] feat-016 waiting_on column and stack frames
- [x] feat-017 Jump to lock owner / thread
- [x] feat-018 Hide JVM noise threads
- [x] feat-019 Cluster threads by stack signature

### What's In Progress

- [ ] None

### What's Next

1. Optional further UX / analysis patterns.

## Decisions Made

- Results UI extracted to `web/src/Results.tsx` + helpers in `analysisUi.ts`.
- Parser retains top 12 stack frames (`ThreadInfo.stack`) while `stack_depth` stays full count.
- Default thread filter: BLOCKED when any exist; hide JVM noise on by default.

## Evidence of Completion

- [x] `cargo test` 17 passed (incl. `captures_top_stack_frames`)
- [x] `./init.sh` green

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
