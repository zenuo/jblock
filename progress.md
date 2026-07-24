# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 12:05
**Active Feature:** feat-022 (done)

## Status

### What's Done

- [x] feat-001 … feat-021 (on main)
- [x] feat-022 Multi-language UI with browser auto-detect

### What's In Progress

- [ ] None on this branch

### What's Next

1. Optional: more locales beyond en/zh.

## Decisions Made

- Lightweight custom i18n (no i18next): `en` + `zh` catalogs under `web/src/i18n/`.
- First visit: `detectBrowserLocale(navigator.languages)`; override stored in `localStorage` (`jblock.locale`).
- HTML export localized; PDF labels stay English (pdf-lib Helvetica cannot embed CJK).

## Evidence of Completion

- [x] `./init.sh` green
- [x] `detectBrowserLocale(['zh-CN']) === 'zh'`, `(['en-US']) === 'en'`

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
