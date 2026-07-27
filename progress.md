# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:00
**Active Feature:** stack preview horizontal overflow fix (main)

## Status

### What's Done

- [x] `.stack-preview` / cluster cards wrap long FQCN frames (`overflow-wrap: anywhere`)
- [x] Cluster head sample-name line also wraps inside the card

### What's In Progress

- [ ] Verify + push main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Prefer wrapping inside the card over horizontal scroll for stack frames (matches thread-table `cell-break` behavior).

## Evidence of Completion

- Pending lint/typecheck
