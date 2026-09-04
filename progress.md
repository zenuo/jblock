# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-043 home points omit Java versions (done)

## Status

### What's Done

- [x] **Home intro: drop "Java 8 · 11 · 17 · 21"**
  - Removed `home.pointVersions` from App, MessageKey, and all 8 locales
  - Help modal still documents Java 8/11/17/21
- [x] Home CTA hairline removed (prior)
- [x] Thread table Id column sort (prior)

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
- Homepage highlights stay at two facts: on-device + supported dump formats. Version coverage remains in Help.

## Evidence of Completion

```text
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
feat-043: home intro omits Java version highlight
```
