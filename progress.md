# Session Progress Log

## Current State

**Last Updated:** 2026-07-26 01:45
**Active Feature:** HTML export non-sticky header (main)

## Status

### What's Done

- [x] Export HTML uses `.app.report`; CSS forces `.app-header` to `position: static` (no sticky overlay)

### What's In Progress

- [ ] None

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Live app keeps sticky header; only exported report opts out via `.report` class so in-app UX is unchanged.

## Evidence of Completion

- Pending lint/typecheck/e2e
