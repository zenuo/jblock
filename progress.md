# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 08:30
**Active Feature:** feat-005 + feat-006 (done)

## Status

### What's Done

- [x] feat-001 Project scaffold & verification (`./init.sh` green)
- [x] feat-002 Thread dump parsing (jstack + ThreadMXBean)
- [x] feat-003 Problem-pattern analysis (state grouping + lock contention)
- [x] feat-004 In-browser result rendering
- [x] feat-005 Format & UX hardening (drag-and-drop, deadlock detection, real-world coverage)
- [x] feat-006 Export HTML (app CSS) / PDF (pdf-lib one page)
- [x] feat-007 Java code generation (lock-contention + deadlock reproducers)
- [x] feat-010 Deadlock cycle detection (done with feat-005)

### What's In Progress

- [ ] None — feat-005 + feat-006 complete.

### What's Next

1. feat-008 Java version support (jenv 8/11/17/21 format diffs).
2. feat-009 ThreadMXBean lock-contention bug fix.

## Blockers / Risks

- [ ] Risk: `pnpm -C web run build` requires `wasm-pack` + the `wasm32-unknown-unknown` target (installed via update script / one-time setup).

## Decisions Made

- **Pure-Rust parser split**: parsing lives in `src/parser.rs` (no wasm deps) so it is host-testable with `cargo test`; `src/lib.rs` only holds bindings.
- **feat-007 codegen**: `src/codegen.rs` emits two scenarios (lock-contention holder + BLOCKED waiters; deadlock via circular `synchronized` acquisition). Count clamped 2..=64. Exposed as `generateJava(scenario, count)` and a CLI `examples/gen_java.rs`.
- **init.sh ordering fix**: `pnpm -C web run wasm` now runs before typecheck/lint (typecheck needs the generated `web/src/wasm/*.d.ts`, which now includes `generateJava`).

## Files Modified This Session (feat-005/006)

- `src/parser.rs` — deadlock detection (`detect_deadlocks`), `Analysis.deadlocks`, robust `is_thread_header`
- `src/lib.rs` — export `Deadlock`
- `tests/fixtures/deadlock_real_jstack.txt` (new) — real javac-21 dump
- `web/src/export.ts` — HTML uses app CSS (`?inline`); PDF via pdf-lib one page
- `web/src/App.tsx` / `index.css` — drag-and-drop, deadlock panel, deadlocks stat
- `web/src/types.ts` — `Deadlock`; `web/package.json` — `pdf-lib`

## Evidence of Completion

- [x] Tests pass: `cargo test` -> 13 passed (5 codegen + 8 parser incl. deadlock + real-world)
- [x] Lint/type/build clean: `./init.sh` green
- [x] Real-world: parses javac-21 deadlock dump, ignores summary preamble, detects 3-thread cycle
- [x] Browser: drag-and-drop analyze, deadlock panel, HTML + PDF export (see PR demo)

## Notes for Next Session

Run `./init.sh` first. Remember WASM is not hot-reloaded: re-run `pnpm -C web run wasm` after editing `src/*.rs`.
