# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 02:55
**Active Feature:** legend fan-layout node spacing (main)

## Status

### What's Done

- [x] Shared `FAN` layout (≥140px center spacing for 96px cards)
- [x] Applied to BusyWait / ConditionStarvation / SyncIo / PoolExhaustion / ConnectionPool demos
- [x] Slightly larger deadlock ring + hot-lock waiter spread; wider legend modal stage

### What's In Progress

- [ ] Verify + push main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Root cause: 96px-wide cards with ~100px center spacing left ~4px gaps. Use 420×260 viewBox and 140px horizontal pitch (~44px edge gap).

## Evidence of Completion

- Pending lint/typecheck/e2e
