# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 17:10
**Active Feature:** feat-035 (done)

## Status

### What's Done

- [x] feat-001 … feat-034
- [x] feat-035 Detect Condition / park starvation

### What's In Progress

- [ ] None on this branch

### What's Next

1. feat-036 Nested lock-order inconsistency risk
2. … see feature_list.json

## Decisions Made

- Reproducer: N threads `Condition.await()` on a shared `ReentrantLock` condition; never `signal`.
- Detect ≥3 WAITING/TIMED_WAITING with ConditionObject.await frames and no RUNNABLE signal/signalAll stack.

## Evidence of Completion

- [x] `./init.sh` green (`cargo test` 56/56; web lint/typecheck/build)

## Notes for Next Session

Start feat-036 next.
