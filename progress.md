# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 13:22
**Active Feature:** feat-030 (done)

## Status

### What's Done

- [x] feat-001 … feat-029
- [x] feat-030 Detect dangerous hot-lock owner

### What's In Progress

- [ ] None on this branch

### What's Next (by benefit)

1. feat-031 connection-pool borrow blocking
2. … see feature_list.json

## Decisions Made

- Detect hottest lock where owner stack has sleep/park/wait/sync-I/O while BLOCKED waiters exist.
- Reproducer `DangerousHotLock`: named `lock-owner` sleeps holding LOCK; `waiter-*` block.
- Pool-exhaustion dumps may also surface this pattern (owner sleeping on shared lock) — intentional overlap.

## Evidence of Completion

- [x] `./init.sh` green (`cargo test` 32/32)

## Notes for Next Session

Start feat-031 next.
