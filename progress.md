# Session Progress Log

## Current State

**Last Updated:** 2026-09-04
**Active Feature:** feat-046 full stack + syntax highlighting

## Status

### What's Done

- [x] Expanding a thread shows **every** stack frame (no 12-frame preview / "more frames")
- [x] Stack frames highlighted: package, class, method, file, line, Native Method
- [x] Same highlighting on stack clusters
- [x] Hide empty waiting_on + multi-column sort (prior)

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
- Tokenize frames locally (no highlighter library). Dark stack-row uses cyan method + yellow line; clusters use accent method on light background.

## Evidence of Completion

```text
$ node --experimental-strip-types --no-warnings scripts/test-stack-frame.mjs
stackFrame tests ok

$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
```

Browser (Dubbo MXBean dump, 648 threads): expand `DubboServerHandler-…-thread-1` (id 138, stack 32) after sorting by Stack. All 32 numbered frames render in the dark pane with cyan methods / yellow line numbers; no "… N more frame(s)". Stack clusters use the same token colors on a light background. Dump-native `…` after ~32 `at` lines is JVM truncation, not UI truncation.
