# Session Progress Log

## Current State

**Last Updated:** 2026-07-26 02:23
**Active Feature:** CJK home title first-line comma (main)

## Status

### What's Done

- [x] Removed leading `—` / `——` from `home.title` in zh / ja / ko
- [x] Added first-line comma/読点: zh `，` / ja `、` / ko `,`
- [x] Latin locales keep editorial em dash unchanged

### What's In Progress

- [x] Commit + push to main

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Prefer deleting the leading dash in CJK hero lines over a CSS decorative bar.
- Connect the two title lines with locale-native commas (zh fullwidth，/ ja 読点、/ ko ASCII ,).

## Evidence of Completion

- Pre-change: `./init.sh` — cargo 88/88; web lint/typecheck/build; e2e 44/44 PASS
- Post dash-removal: `pnpm -C web run lint` + `typecheck` PASS
