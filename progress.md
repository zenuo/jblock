# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:20
**Active Feature:** feat-047 Legend peer sample of 3 + copyable names (main)

## Status

### What's Done

- [x] Peer/fan legends capped at 3 equal nodes + total caption when `peerTotal > 3`
- [x] Hover tip with selectable full thread name
- [x] Deadlock cycles left as sequential layouts
- [x] `./init.sh` **47/47 PASS**

### What's In Progress

- [x] Commit + push main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Peer demos: busy-wait, condition, sync-io, pool, connection waiters, hot-lock waiters, blocked, clean (+ aliases).
- Not peer-capped: deadlock / future-latch / lock-order cycles.

## Evidence of Completion

- `./init.sh`: cargo 89/89; web lint/typecheck/build; e2e **47/47 PASS** (incl. feat-047)
