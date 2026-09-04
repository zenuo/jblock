# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-044 toolbar filename vertical align (done)

## Status

### What's Done

- [x] **Toolbar dump filename vertical centering** (feat-044 polish on `main`)
  - `.workspace-toolbar` uses `align-items: center`
  - `.dump-filename` is a flex item with `align-self: center` and matching `min-height`
  - e2e feat-044 static check asserts the CSS rules
- [x] **feat-061** Thread-dump flame graph at the end of results and HTML export
- [x] **feat-062** Collapsible left report section nav
- [x] **feat-060** Live monitor-contention dumps from generated Java
- [x] **feat-059** Dedupe reentrant held locks (prior)

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

- Stayed on `main` (user: 「main上调整」 + AGENTS.md).
- Filename sat high because the toolbar is flex with default `stretch`; the span filled button height and the text stuck to the top.
- Worked as a CSS polish of feat-044, not a new feature id.

## Evidence of Completion

```text
$ node scripts/e2e-features.mjs --skip-web
Summary: 62/62 features PASS
feat-044 static: toolbar dump filename is vertically centered — ok

$ puppeteer getBoundingClientRect (sample.txt and Dubbo_JStack.log.2026-09-02_13_13_35.txt)
filenameTopGap === filenameBottomGap === 10.1875
midDeltaVsToolbar === 0
midDeltaVsButton === 0
```
