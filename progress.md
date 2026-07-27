# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 13:20
**Active Feature:** docs — work directly on `main` (branching policy)

## Status

### What's Done

- [x] Diagnosed why “直接在 main 上改” never took effect: AGENTS.md had trunk-based + short-lived PR wording, and explicitly told Cloud agents to keep using `cursor/...` branches
- [x] Rewrote `AGENTS.md` Branching Model: default commit/push on `main`; no auto-branch / no default PR unless human asks

### What's In Progress

- [ ] Confirm `./init.sh` still green after docs-only change

### What's Next

1. Commit when asked (docs-only)
2. Note: Cursor Cloud system defaults may still try to open `cursor/...` branches; AGENTS.md now explicitly overrides that habit for this repo

## Decisions Made

- Prefer direct-on-`main` over short-lived feature branches for this solo/trunk workflow.
- Branches/PRs only when the human explicitly requests them.

## Evidence of Completion

- `AGENTS.md` Branching Model section replaced (Work on `main`)
- Baseline: `./init.sh` (running / pending)
