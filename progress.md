# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 12:45
**Active Feature:** feat-026 (in progress)

## Status

### What's Done

- [x] feat-001 … feat-025 (on main)
- [ ] feat-026 Language icon menu + more locales

### What's In Progress

- [x] Locale catalogs: pt / es / nl / fr / ja / ko (+ existing en / zh)
- [x] Globe icon language menu component
- [ ] Verification (`./init.sh` + UI check)

### What's Next

1. Finish feat-026 verification and open PR.

## Decisions Made

- Language control is an icon button (globe SVG) opening a dropdown listbox; no text select.
- Locales live under `web/src/i18n/locales/*.ts`; catalogs assembled in `messages.ts`.
- Browser detect extended for pt/es/nl/fr/ja/ko prefixes; `htmlLangFor` shared by app + HTML export.

## Evidence of Completion

- [ ] `./init.sh` green

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
