# Session Progress Log

## Current State

**Last Updated:** 2026-07-26 02:10
**Active Feature:** CJK home title dash removal (main)

## Status

### What's Done

- [x] Removed leading `—` / `——` from `home.title` in zh / ja / ko (serif CJK confuses with 一 / ー)
- [x] Latin locales keep editorial em dash unchanged
- [x] Baseline `./init.sh` green before edit (44/44 features)

### What's In Progress

- [ ] Post-change lint/typecheck evidence

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Prefer deleting the leading dash in CJK hero lines over a CSS decorative bar (simpler; no 一/ー ambiguity).
- Scope limited to zh/ja/ko `home.title`; help/body copy that uses mid-sentence 破折号 is unchanged.

## Evidence of Completion

- Pre-change: `./init.sh` — cargo 88/88; web lint/typecheck/build; e2e 44/44 PASS
