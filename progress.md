# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 14:40
**Active Feature:** feat-038 (done)

## Status

### What's Done

- [x] feat-001 … feat-037
- [x] feat-038 Detect Thread.sleep-as-scheduler anti-pattern

### What's In Progress

- [ ] None on this branch

### What's Next

1. feat-039 framework worker-pool saturation (Tomcat/Jetty/Netty)
2. … see feature_list.json

## Decisions Made

- Skip JVM-noise thread names (mirror of `isJvmNoise`).
- Require ≥3 TIMED_WAITING business threads sharing a top-3 stack signature with `Thread.sleep` / `sleep0`.
- Exclude Condition.await stacks (feat-035) to avoid overlap.
- Reproducer: `SleepAsScheduler.scheduleNextTick()` sleeps forever in a named loop.

## Evidence of Completion

- `./init.sh` green — cargo lib tests + web lint/typecheck/build OK
- Fixture: `tests/fixtures/patterns/sleep_as_scheduler_jstack.txt`
- Live capture: `live_capture_sleep_as_scheduler_detects_pattern`
