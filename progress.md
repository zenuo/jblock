# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-065 flame live tooltip (no native SVG title)

## Status

### What's Done

- [x] Live flame graph hover uses only `.flame-tip` (removed duplicate SVG `<title>`)
- [x] HTML export still uses `<title>` for static hover
- [x] Flame group-by stack/state and fullscreen (prior)

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
- Keep SVG `<title>` only on the exported static SVG.

## Evidence of Completion

```text
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
Summary: 65/65 features PASS
```

Browser: hover Unsafe.park for 4+ seconds — only the custom dark .flame-tip; no native second box.
