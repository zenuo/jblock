# Session Progress Log

## Current State

**Last Updated:** 2026-09-02
**Active Feature:** feat-061 + feat-062 (done)

## Status

### What's Done

- [x] **feat-061** Thread-dump flame graph at the end of results and HTML export
  - Each thread is one sample; width = folded thread count
  - Stacks reversed so oldest frames sit at the bottom
  - Live: hover tooltip + click-to-zoom; export: static SVG with `<title>`
- [x] **feat-062** Collapsible left report section nav
  - Indexes findings / deadlocks / contention / states / clusters / threads / flame graph
  - Click jumps to `section-*`; bottom button collapses; HTML export uses a checkbox/label
- [x] **feat-060** Live monitor-contention dumps from generated Java (landed on main in parallel)
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

- No extra JS chart library: fold + SVG layout live in `web/src/flamegraph.ts` (zero new deps).
- Live flame graph respects "Hide JVM noise"; HTML export includes every thread (same as the export threads table).
- Sidebar is sticky inside the results workspace (not a viewport-fixed overlay) so the home intro stays unchanged.
- Work stayed on `main` per AGENTS.md / user request.
- After rebase, flame/nav are feat-061/062 because feat-060 was taken by live contention dumps.

## Evidence of Completion

```text
$ node --experimental-strip-types --no-warnings scripts/test-flamegraph.mjs
flamegraph unit tests ok

$ cargo test --features cli
test result: ok. 111 passed

$ pnpm -C web run lint && pnpm -C web run typecheck && pnpm -C web run build
✓ lint / typecheck / vite build

$ node scripts/e2e-features.mjs --skip-web
Summary: 62/62 features PASS (incl. feat-061, feat-062)
```
