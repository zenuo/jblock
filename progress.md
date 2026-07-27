# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:15
**Active Feature:** feat-047 Legend peer sample of 3 + copyable names (main)

## Status

### What's Done

- [x] Inventory: peer/fan demos (busy-wait, condition, sync-io, pool, connection waiters, hot-lock waiters, blocked, clean); deadlock cycles excluded
- [x] Cap peer nodes at 3; `PeerSampleNote` when `peerTotal > 3`
- [x] Hover tip with `user-select: all` full thread name (foreignObject)
- [x] `FindingActors.peerTotal` + i18n `legend.peerSample` (8 locales)
- [x] feat-047 + e2e checks

### What's In Progress

- [ ] Full `./init.sh` + push main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- 4th lower fan card was misread as a special role; equal peers stay in one row of 3.
- Deadlock/future-latch/lock-order remain cycle layouts (not peer-capped).

## Evidence of Completion

- Pending `./init.sh`
