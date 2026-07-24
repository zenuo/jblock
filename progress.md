# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 10:00
**Active Feature:** feat-020 (done; rebased onto main with feat-013..019)

## Status

### What's Done

- [x] feat-001 … feat-012 (on main)
- [x] feat-013 Problem-first findings summary
- [x] feat-014 Aggregate lock contention by lock
- [x] feat-015 Thread table filter and sort
- [x] feat-016 waiting_on column and stack frames
- [x] feat-017 Jump to lock owner / thread
- [x] feat-018 Hide JVM noise threads
- [x] feat-019 Cluster threads by stack signature
- [x] feat-020 GitHub Actions CI/CD

### What's In Progress

- [ ] None on this branch

### What's Next

1. Enable GitHub Pages source = GitHub Actions in repo settings after merge.

## Decisions Made

- Results UI extracted to `web/src/Results.tsx` + helpers in `analysisUi.ts`.
- Parser retains top 12 stack frames (`ThreadInfo.stack`) while `stack_depth` stays full count.
- Default thread filter: BLOCKED when any exist; hide JVM noise on by default.
- CI mirrors `./init.sh`: cargo test + pnpm wasm/lint/typecheck/build.
- CD deploys `web/dist` to GitHub Pages only on `main` push.
- Vite `base` comes from `VITE_BASE` (CI sets `/jblock/` for project Pages).

## Evidence of Completion

- [x] `cargo test` 17 passed (incl. `captures_top_stack_frames`)
- [x] `.github/workflows/ci.yml` present
- [x] `VITE_BASE=/jblock/ pnpm -C web run build` produces `/jblock/` asset paths
- [x] `./init.sh` green

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
After merging, turn on Pages → Build and deployment → Source: GitHub Actions.
