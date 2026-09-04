# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-015/016 thread table hide empty waiting_on + multi-sort

## Status

### What's Done

- [x] Hide **Waiting on** when every visible row is blank (live table + HTML export)
- [x] Multi-column sort: click replaces, Shift-click adds/toggles; headers show ↑/↓ and rank
- [x] Home intro Java version line removed (prior)
- [x] Home CTA hairline removed (prior)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- Very deep stacks are capped at 40 frames from the root (oldest side) so SVG height stays bounded.
- HTML export sidebar collapse uses `:has(+ checkbox)` and needs a reasonably modern browser.

## Decisions Made

- Stayed on `main` per AGENTS.md.
- Hide waiting_on based on the currently filtered rows, not the whole dump.
- Shift-click is the additive sort gesture (spreadsheet convention).

## Evidence of Completion

```text
$ node --experimental-strip-types --no-warnings scripts/test-sort-threads.mjs
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
```
