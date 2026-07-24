# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 12:20
**Active Feature:** feat-024 (done)

## Status

### What's Done

- [x] feat-001 … feat-023 (on main)
- [x] feat-024 Website favicon

### What's In Progress

- [ ] None on this branch

### What's Next

1. Optional: raster apple-touch-icon if needed for iOS home-screen.

## Decisions Made

- SVG favicon in `web/public/favicon.svg` (Vite copies `public/` to dist root).
- Motif: indigo→violet brand tile + interlocking thread blocks with a wait-for edge (matches jblock purpose).
- Linked via `<link rel="icon" type="image/svg+xml">` plus `theme-color`.

## Evidence of Completion

- [x] `./init.sh` green
- [x] Production build includes `dist/favicon.svg`

## Notes for Next Session

Run `./init.sh` first. After `src/*.rs` edits, re-run `pnpm -C web run wasm`.
