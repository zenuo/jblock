# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 14:55
**Active Feature:** feat-039 (done)

## Status

### What's Done

- [x] feat-001 … feat-038
- [x] feat-039 Detect framework worker-pool saturation (Tomcat/Jetty/Netty)

### What's In Progress

- [ ] None on this branch

### What's Next

1. feat-040 DNS / name-resolution stall clusters
2. … see feature_list.json

## Decisions Made

- Families: Tomcat `http-nio-*-exec-*`, Jetty `qtp*`, Netty `*nioEventLoop*`.
- Idle stacks excluded: getTask / TaskQueue.take / QueuedThreadPool.idleJob / Selector.select / epollWait.
- Saturation = ≥3 same-family non-idle workers sharing a top-4 stack signature.
- Reproducer uses Tomcat-style names on a shared LOCK (detector covers all three families).

## Evidence of Completion

- `./init.sh` green — cargo lib tests + web lint/typecheck/build OK
- Fixture: `tests/fixtures/patterns/framework_pool_saturation_jstack.txt`
- Live capture: `live_capture_framework_pool_saturation_detects_pattern`
