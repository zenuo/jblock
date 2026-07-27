# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 03:45
**Active Feature:** sticky legend thread name for copy (feat-047 fix)

## Status

### What's Done

- [x] Root cause: tip cleared on `ActorLabel` / stage `mouseLeave`, so moving to select text removed the name
- [x] Tip is sticky after hover/focus/click; updates when another node is selected; clears only when legend kind changes
- [x] e2e feat-047 guards against `setTip(null)` / `setHoverThread(null)` on mouseLeave
- [x] `./init.sh` 47/47 PASS
- [x] Browser: tip stays after mouse leave; text selectable

### What's In Progress

- [x] Commit + push + PR #24

### What's Next

1. Merge PR when ready

## Decisions Made

- Sticky last-hovered name (not hover-only) so users can move to the tip and copy with `user-select: all`.
- No separate copy button; selectable tip is enough and matches feat-047.

## Evidence of Completion

- `./init.sh` → Summary: 47/47 features PASS
- Browser: Load sample → Deadlock/Hottest lock Legend → hover node → leave → tip still shows → triple-click selects name
- Artifacts:
  - `/opt/cursor/artifacts/screenshots/legend-tip-hover.webp`
  - `/opt/cursor/artifacts/screenshots/legend-tip-sticky-after-leave.webp`
  - `/opt/cursor/artifacts/screenshots/legend-tip-text-selected.webp`
  - `/opt/cursor/artifacts/screenshots/legend-tip-hotlock-sticky.webp`
