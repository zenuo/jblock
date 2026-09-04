# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-063 flame graph fullscreen

## Status

### What's Done

- [x] Flame graph toolbar Fullscreen / Exit fullscreen (viewport overlay)
- [x] Escape + body scroll lock; graph relayouts via ResizeObserver
- [x] Full thread stacks + syntax highlighting (prior)

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- Very deep stacks are capped at 40 frames from the root (oldest side) so SVG height stays bounded.
- HTML export sidebar collapse uses `:has(+ checkbox)` and needs a reasonably modern browser.
- Flame fullscreen is live-page only; exported HTML stays a static SVG.

## Decisions Made

- Stayed on `main` per AGENTS.md.
- CSS viewport overlay (not native Fullscreen API) so Escape, chrome, and ResizeObserver stay under app control.

## Evidence of Completion

```text
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
Summary: 63/63 features PASS
```

Browser: sample dump → Flame graph → Fullscreen covers the viewport (heading + Exit fullscreen); Exit button and Escape both restore the inline panel.
