# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:00
**Active Feature:** stack preview horizontal overflow fix (main)

## Status

### What's Done

- [x] `.stack-preview` / cluster cards wrap long FQCN frames (`overflow-wrap: anywhere`)
- [x] Cluster head sample-name line also wraps inside the card
- [x] e2e feat-019 asserts wrap styles; lint/typecheck/e2e 46/46 PASS

### What's In Progress

- [x] Commit + push main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Prefer wrapping inside the card over horizontal scroll for stack frames (matches thread-table `cell-break` behavior).

## Evidence of Completion

- `pnpm -C web run lint` + `typecheck` PASS; `node scripts/e2e-features.mjs --skip-web` **46/46 PASS**
