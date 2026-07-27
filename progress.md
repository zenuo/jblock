# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 02:45
**Active Feature:** feat-046 Full thread stack frames (main)

## Status

### What's Done

- [x] Parser retains full stack (removed `MAX_STACK_FRAMES`)
- [x] UI preview (12) + clickable "… N more" to reveal entire stack
- [x] `feat-046` + e2e checks + `captures_full_stack_frames` cargo test

### What's In Progress

- [ ] Full `./init.sh` verification

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Keep a 12-frame preview when expanded so deep stacks stay scannable; "show all" reveals the rest (frames are now available from the parser).

## Evidence of Completion

- Pending `./init.sh`
