# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 13:05
**Active Feature:** feat-027 / feat-028 (done)

## Status

### What's Done

- [x] feat-001 … feat-026
- [x] feat-027 Pattern reproducer + dump capture harness
- [x] feat-028 Detect thread-pool exhaustion

### What's In Progress

- [ ] None on this branch

### What's Next (by benefit)

1. feat-029 sync I/O / RPC hotspot clusters
2. feat-030 dangerous hot-lock owner
3. feat-031 connection-pool borrow blocking
4. … see feature_list.json feat-029..041

## Decisions Made

- Patterns recorded as feat-027..041 sorted by benefit.
- `Analysis.patterns: Vec<PatternHit>` for higher-level hits.
- Live JDK capture in `src/capture.rs`; offline fixture under `tests/fixtures/patterns/`.
- Refresh fixtures with `JBLOCK_UPDATE_FIXTURES=1 cargo test`.

## Evidence of Completion

- [x] `cargo test` 23/23 (includes live jstack capture)
- [x] web lint / typecheck after wasm rebuild

## Notes for Next Session

Start feat-029 next. Reuse capture harness + PatternHit pipeline.
