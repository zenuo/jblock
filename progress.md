# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-043 home points hairline removed (done)

## Status

### What's Done

- [x] **Home intro: remove hairline under CTA**
  - `.home-points` had `border-top`; `.controls { margin-bottom: 0 }` cancelled the CTA gap, so the rule sat on the buttons
  - Divider removed; `.home-cta.controls` restores spacing with margin
- [x] Thread table Id column sort (feat-015)
- [x] Toolbar dump filename vertical centering

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
- Prefer whitespace over a rule between the home CTA and the three capability points.

## Evidence of Completion

```text
$ node scripts/e2e-features.mjs --skip-web
feat-043 static: home points use spacing, not a hairline rule
```
