# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 09:30
**Active Feature:** feat-012 (done)

## Status

### What's Done

- [x] feat-001 … feat-011
- [x] feat-012 Codegen entry as top-right modal

### What's In Progress

- [ ] None

### What's Next

1. Optional UX polish / new features.

## Decisions Made

- **feat-012**: Main page is dump analysis only. Java reproducer opens from header top-right via modal (Escape, backdrop click, ×). Codegen errors stay local to the modal.

## Evidence of Completion

- [x] Inline `.panel.codegen` removed from main flow
- [x] Header button `data-testid="open-codegen"` + modal `data-testid="codegen-modal"`
- [x] `./init.sh` (pending)

## Notes for Next Session

Run `./init.sh` first.
