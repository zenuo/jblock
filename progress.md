# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-015 thread table Id column sort (done)

## Status

### What's Done

- [x] **Thread table Id column sort** (feat-015 polish on `main`)
  - Clicking **Id** sorts numerically (9 → 10 → 100), not lexicographically
  - Missing ids stay last in both directions
  - Second click toggles desc; same pattern as Name
- [x] Toolbar dump filename vertical centering (prior)
- [x] feat-061 / feat-062 flame graph + report nav (prior)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock
3. Optional: flame-graph color-by-package or export click-to-zoom

### Unresolved Risks

- Very deep stacks are capped at 40 frames from the root (oldest side) so SVG height stays bounded.
- HTML export sidebar collapse uses `:has(+ checkbox)` and needs a reasonably modern browser.

## Decisions Made

- Stayed on `main` per AGENTS.md.
- Id values are strings in `ThreadInfo`; sort parses numbers when both sides are finite, otherwise `localeCompare({ numeric: true })`.
- HTML export table stays static (no click-to-sort).

## Evidence of Completion

```text
$ node --experimental-strip-types --no-warnings scripts/test-sort-threads.mjs
sortThreads tests ok

$ pnpm -C web run lint && pnpm -C web run typecheck
✓ lint / typecheck

$ node scripts/e2e-features.mjs --skip-web
Summary: 62/62 features PASS (feat-015 includes id sort)
```
