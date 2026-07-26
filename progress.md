# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 02:25
**Active Feature:** feat-045 Non-sticky app header (main)

## Status

### What's Done

- [x] Removed `position: sticky` (+ frosted sticky chrome) from `.app-header`
- [x] Added `feat-045` to `feature_list.json`
- [x] Added e2e static checks asserting `.app-header` is not sticky/fixed
- [x] `./init.sh` green (45/45 features)

### What's In Progress

- [x] Commit + push to main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Header stays in normal document flow so scroll moves it off-screen with content.
- Dropped sticky-only backdrop blur / translucent background; kept has-results bottom border.

## Evidence of Completion

- `./init.sh`: cargo 88/88; web lint/typecheck/build; e2e **45/45 PASS** (incl. feat-045)
