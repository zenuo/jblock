# Session Progress Log

## Current State

**Last Updated:** 2026-08-03
**Active Feature:** feat-056 (done)

## Status

### What's Done

- [x] **feat-056** CLI shell (file / stdin / clipboard)
  - Optional Cargo feature `cli` + `[[bin]] jblock` (`required-features = ["cli"]`)
  - Inputs: positional files, stdin pipe (non-TTY / `-`), `--clipboard` via OS tools
  - Outputs: Findings-first text (default sections) or `--json` / `-j`
  - Filters: `--section`, `--state`, `--severity`, `--hide-jvm`, `-v`, `-n/--limit`, `--color`, `-q`
  - Exit codes: 0 / 1 / 2 / 3
  - Clipboard uses `pbpaste` / `wl-paste` / `xclip` / `xsel` (no arboard; rustc 1.83 MSRV)

### What's In Progress

- [ ] (none)

### What's Next

1. User feedback / optional polish (HTML export from CLI, install script, man page)

## Decisions Made

- Keep `cli` off default features so wasm-pack / Pages builds stay unchanged
- Pin `clap = 4.5.41` for rustc 1.83; avoid clap 4.6+ (needs 1.85) and arboard/image (edition2024)
- Clipboard via external tools instead of arboard to keep host deps light
- Text report mirrors web Findings-first layout; no "clean" placeholder (feat-054 parity)
- Work on `main` per AGENTS.md branching model

## Evidence of Completion

```text
$ ./init.sh
cargo test --features cli: 105 passed
pnpm lint/typecheck/build: green
e2e: Summary: 56/56 features PASS (incl. feat-056)

$ target/debug/jblock tests/fixtures/deadlock_real_jstack.txt; echo $?
# prints FINDINGS/SUMMARY/CONTENTION/DEADLOCKS; exit 1

$ echo 'not a dump' | target/debug/jblock; echo $?
# exit 3
```
