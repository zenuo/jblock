# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 13:32
**Active Feature:** feat-031 (done)

## Status

### What's Done

- [x] feat-001 … feat-030
- [x] feat-031 Detect connection-pool borrow blocking

### What's In Progress

- [ ] None on this branch

### What's Next

1. feat-032 Future/Latch wait trees
2. … see feature_list.json

## Decisions Made

- Lightweight mock `HikariDataSource` inside `ConnectionPoolStarve` (no external deps).
- Detect ≥3 threads in WAITING/TIMED_WAITING/BLOCKED with Hikari/DBCP/Druid/getConnection/borrowObject frames.

## Evidence of Completion

- [x] `./init.sh` green (`cargo test` 36/36)

## Notes for Next Session

Start feat-032 next.
