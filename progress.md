# Session Progress Log

## Current State

**Last Updated:** 2026-07-25 15:35
**Active Feature:** feat-044 (done)

## Status

### What's Done

- [x] Chose Web Crypto SHA-256 (option C) over MD5 / Rust
- [x] Toolbar shows dump filename after results; hover = SHA-256
- [x] HTML export source line `title` shows SHA-256
- [x] feat-044 + e2e static checks

### What's In Progress

- [ ] None

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip

## Decisions Made

- Digest = SHA-256 of dump UTF-8 text (`TextEncoder` + `crypto.subtle`), matching analyzed content.
- Computed in JS at analyze time; not in WASM.

## Evidence of Completion

- lint + typecheck pass; e2e **44/44 PASS**
