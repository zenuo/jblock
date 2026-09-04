# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-064 flame graph group by stack or state

## Status

### What's Done

- [x] Flame graph defaults to stack grouping (no Thread.State row)
- [x] Toolbar Stack / State segmented control; preference persisted
- [x] Flame graph fullscreen (prior)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- Very deep stacks are capped at 40 frames from the root (oldest side) so SVG height stays bounded.
- HTML export sidebar collapse uses `:has(+ checkbox)` and needs a reasonably modern browser.
- Flame fullscreen is live-page only; exported HTML stays a static SVG (stack grouping).

## Decisions Made

- Stayed on `main` per AGENTS.md.
- Default grouping is classic stack merge; dump-style state split is opt-in.

## Evidence of Completion

```text
$ node --experimental-strip-types --no-warnings scripts/test-flamegraph.mjs
flamegraph unit tests ok
$ pnpm -C web run lint && pnpm -C web run typecheck
Browser: sample dump flame graph defaults to Stack (`all` then `Thread.run`); State inserts BLOCKED/RUNNABLE/WAITING/TIMED_WAITING above `all`; switching back merges stacks again.
