# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 14:15
**Active Feature:** feat-048 / feat-049 (pending — acceptance only)

## Status

### What's Done

- [x] Added **feat-048** Format-specialized thread block parsing (`pending`) with `acceptance[]`
- [x] Added **feat-049** Virtual thread dump support (codegen-tested) (`pending`) with `acceptance[]`
- [x] e2e matrix skips non-`done` features when `acceptance[]` is present (keeps `./init.sh` green)

### What's In Progress

- [ ] Implement feat-048 or feat-049 (not started)

### What's Next

1. **feat-048**: branch `analyze` on `DumpFormat` into jstack vs MXBean block parsers; remove dual-try `extract_id` / dual lock regexes on known formats
2. **feat-049**: JSON `Thread.dump_to_file` parser + codegen Scenario + live/fixture cargo tests (independent of feat-048; nicer after it)

## Decisions Made

- Two features are independent: feat-049 depends on feat-002 + feat-027 only (not on feat-048)
- Acceptance criteria live in `feature_list.json` `acceptance` arrays; evidence stays empty until done
- Pending features must record non-empty `acceptance[]` or e2e fails that entry

## Evidence of Completion

- `feature_list.json`: feat-048, feat-049 `status: pending` + acceptance checklists
- `scripts/e2e-features.mjs`: pending-feature skip path
