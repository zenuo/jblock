# Session Progress Log

## Current State

**Last Updated:** 2026-09-02
**Active Feature:** feat-060 (done)

## Status

### What's Done

- [x] **feat-060** Live monitor-contention dumps from generated Java
  - LockContention now starts a named `holder`, then `waiter-*` (CountDownLatch)
  - Optional `-Djblock.mxbean.dump` writes ThreadMXBean.dumpAllThreads(true, true)
  - Live tests capture jstack + MXBean; fixtures under `tests/fixtures/patterns/`
  - Parser asserts waiter → same lock → owner `holder`

- [x] **feat-059** Dedupe reentrant held locks (prior)
- [x] **feat-058** Remove unreliable Java version detection (prior)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group

## Decisions Made

- No new PatternKind: monitor contention is `blocked_edges` (UI 锁竞争 already consumes it)
- 1 holder + N-1 waiters (count=3 → 2 waiters), matching java-versions contention dumps
- MXBean dump is written by the Java process itself so the fixture is true ThreadInfo#toString format

## Evidence of Completion

```text
$ cargo test --features cli --lib
111 passed  (incl. live_capture_lock_contention_{,mxbean_}detects_edges)

$ cargo run --features cli --bin jblock -- tests/fixtures/patterns/lock_contention_jstack.txt
CONTENTION
  lock 0x0000000715614fa0  owner=holder  waiters=2
    <- waiter-0
    <- waiter-1

$ ./init.sh
e2e: 60/60 PASS (feat-060)
```
