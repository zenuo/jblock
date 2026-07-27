# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:35
**Active Feature:** sticky legend thread name for copy (feat-047 fix)

## Status

### What's Done

- [x] Root cause: tip cleared on `ActorLabel` / stage `mouseLeave`, so moving to select text removed the name
- [x] Tip is sticky after hover/focus/click; updates when another node is selected; clears only when legend kind changes
- [x] e2e feat-047 guards against `setTip(null)` / `setHoverThread(null)` on mouseLeave

### What's In Progress

- [ ] Browser verify copy stays after mouse leave
- [ ] `./init.sh` / commit + PR

### What's Next

1. Browser: Load sample → Busy-wait Legend → hover node → leave → tip still shows → select/copy
2. Commit, push, open PR

## Decisions Made

- Sticky last-hovered name (not hover-only) so users can move to the tip and copy with `user-select: all`.
- No separate copy button; selectable tip is enough and matches feat-047.

## Evidence of Completion

- (pending browser + `./init.sh`)
