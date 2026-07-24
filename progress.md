# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 12:30
**Active Feature:** feat-025 (in progress → verifying)

## Status

### What's Done

- [x] feat-001 … feat-024 (on main)
- [x] feat-025 Richer Load sample dump (implementation)

### What's In Progress

- [ ] feat-025 verification (`./init.sh`)

### What's Next

1. Merge feat-025 after CI green.

## Decisions Made

- Shared source of truth: `web/src/sample.tdump` (Vite `?raw` import + Rust `include_str!` test).
- Sample intentionally demos: 3-thread deadlock, 4-waiter hot lock, stack clusters, JVM noise, mixed states.

## Evidence of Completion

- [ ] `./init.sh` green
- [x] `cargo test parses_web_sample_dump` pass

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
