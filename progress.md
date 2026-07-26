# Session Progress Log

## Current State

**Last Updated:** 2026-07-26 02:00
**Active Feature:** legend HotLockDemo placeholders + color match (main)

## Status

### What's Done

- [x] Removed fake W1/W2/W3 placeholders from HotLockDemo (was why "W2" missing from dump)
- [x] Waiter nodes use red (`#ef4444` / `#fee2e2`) to match `swatch-waiter` legend key
- [x] Finalizer actors keep `nodes` for real-name fallback when app waiters absent

### What's In Progress

- [ ] Verification

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- W1/W2/W3 were synthetic fallbacks when `actors.waiters` was empty (common for finalizer-only hits); legend must use dump thread names only.
- Diagram waiter color aligned to the red legend swatch (not amber).

## Evidence of Completion

- Pending lint/typecheck/e2e
