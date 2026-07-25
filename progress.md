# Session Progress Log

## Current State

**Last Updated:** 2026-07-25 15:20
**Active Feature:** fix modal scrollbar layout shift (main)

## Status

### What's Done

- [x] `lockBodyScroll()` compensates scrollbar width via `padding-right`
- [x] Wired into PatternLegendModal + HelpModal (same overflow-hidden shift)

### What's In Progress

- [ ] None

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip for zero-toolchain local deploy

## Decisions Made

- Root cause: `body { overflow: hidden }` removes the vertical scrollbar and widens the layout. Fix by adding matching `padding-right` while locked.

## Evidence of Completion

- Pending lint/typecheck
