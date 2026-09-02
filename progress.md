# Session Progress Log

## Current State

**Last Updated:** 2026-09-02
**Active Feature:** feat-059 (done)

## Status

### What's Done

- [x] **feat-059** Dedupe reentrant held locks
  - ThreadMXBean / jstack emit one `- locked` annotation per nested `synchronized` frame
  - `held_locks` now keeps each unique monitor once (innermost-first dump order)
  - Screenshot of Dubbo MXBean dump repeating `JDBC4Connection@…` 2–3 times was a display of reentrant entries, not N distinct locks

- [x] **feat-058** Remove unreliable Java version detection (prior session)
  - Root cause: jstack header `Full thread dump … VM (25.45-b02 mixed mode)` is the HotSpot VM token for Java 8u45, not Java 25
  - Withdrawn: `detect_java_version`, `Analysis.java_version`, Findings/HTML/CLI badges

- [x] **feat-057** CI cross-platform CLI artifacts (prior session)
- [x] **feat-056** CLI shell (prior session)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: show reentrancy count (`×2`) next to a unique lock if operators want hold-count

### Unresolved Risks

- None for feat-059. Distinct locks on the same thread still list separately.

## Decisions Made

- Deduplicate at parse time (all three block parsers), not only in the threads table, so CLI JSON / HTML export / lock-order analysis stay consistent
- Keep first-seen (innermost) order; `lock_acquisition_order` still reverses to outermost-first
- Do not add a reentrancy counter in this batch
- Do not guess a Java product version from dump text (feat-058)

## Evidence of Completion

```text
$ cargo test --features cli --lib dedupes_reentrant_held_locks
test parser::tests::jstack_dedupes_reentrant_held_locks ... ok
test parser::tests::mxbean_dedupes_reentrant_held_locks ... ok

$ cargo run --features cli --bin jblock -- -j --section threads Dubbo_JStack.log
# Id=664 held_locks = [ReadAheadInputStream@3ee52c67, JDBC4Connection@3022c5cb]  (was 3 lines)
# Id=663 held_locks = [ReadAheadInputStream@341d4717, JDBC4Connection@711fcff0]  (was 4 lines)

$ node scripts/e2e-features.mjs --skip-web
Summary: 59/59 features PASS (incl. feat-059)
```
