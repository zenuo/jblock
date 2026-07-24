# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 12:15
**Active Feature:** feat-023 (done; landed on main)

## Status

### What's Done

- [x] feat-001 … feat-022 (on main)
- [x] feat-023 Pattern legend demo modals

### What's In Progress

- [ ] None

### What's Next

1. Optional: more locales / richer per-dump diagram using real thread names.

## Decisions Made

- Each finding has a `kind` (`deadlock` | `hot-lock` | `blocked` | `clean`).
- Legend button opens `PatternLegendModal` with animated SVG demos.
- Respect `prefers-reduced-motion`.
- Committed directly to `main` per user request.

## Evidence of Completion

- [x] `./init.sh` green

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
