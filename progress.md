# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 09:05
**Active Feature:** feat-009 (done)

## Status

### What's Done

- [x] feat-001 … feat-008 (see prior entries)
- [x] feat-009 ThreadMXBean lock-contention bug fix (`Class@hash` / `blocked on` / `locked`)
- [x] feat-010 Deadlock cycle detection (done with feat-005)

### What's In Progress

- [ ] None — backlog complete for feat-001..010.

### What's Next

1. Optional polish / new features beyond the current backlog.

## Blockers / Risks

- [ ] Risk: `pnpm -C web run build` requires `wasm-pack` + the `wasm32-unknown-unknown` target.

## Decisions Made

- **feat-009 MXBean locks**: parse `-  blocked on Class@hash` and `-  locked Class@hash`; ignore `-  waiting on` (Condition/park). Header `BLOCKED on … owned by "…"` is a fallback for waiting_on / owner when monitor lines are incomplete. Lock identity keeps the full `Class@hash` string so waiter and holder match.
- Real-world evidence: Flink/Kafka dump `tdump_15c7` → fixture excerpt `tests/fixtures/mxbean_real_contention.txt`; full dump yields 68 blocked edges (66 on RollingFileManager@30dbe1cc, 1 kafka Object@7ec4e9a), all with owners.

## Files Modified This Session (feat-009)

- `src/parser.rs` — MXBean lock parsing + owned-by fallback; new tests
- `tests/fixtures/mxbean_real_contention.txt` — excerpt from uploaded dump
- `tests/fixtures/java-versions/FORMAT_DIFFS.md` — mark MXBean locks done
- `feature_list.json` / `progress.md` / `session-handoff.md`

## Evidence of Completion

- [x] `cargo test` → 16 passed (incl. `detects_mxbean_format_lock_contentions` + real-world)
- [x] Full dump smoke: 3962 threads, 68/68 BLOCKED edges with owners
- [x] `./init.sh` (run before claiming done)

## Notes for Next Session

Run `./init.sh` first. WASM is not hot-reloaded after `src/*.rs` edits.
