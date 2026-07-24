# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 18:10
**Active Feature:** feat-036 (done)

## Status

### What's Done

- [x] feat-001 … feat-035
- [x] feat-036 Detect nested lock-order inconsistency risk

### What's In Progress

- [ ] None on this branch

### What's Next

1. feat-037 Finalizer / Reference Handler pressure
2. … see feature_list.json

## Decisions Made

- Build lock-order edges from nested `locked` frames (reverse dump order) + hold-while-waiting.
- Report when both A→B and B→A are observed across ≥2 threads.
- Reproducer: classic opposite-order `LOCK_A`/`LOCK_B` handshake that deadlocks.

## Evidence of Completion

- [ ] `./init.sh` (pending this turn)

## Notes for Next Session

Start feat-037 next.
