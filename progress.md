# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 08:05
**Active Feature:** feat-007 - java code generation (done)

## Status

### What's Done

- [x] feat-001 Project scaffold & verification (`./init.sh` green)
- [x] feat-002 Thread dump parsing (jstack + ThreadMXBean)
- [x] feat-003 Problem-pattern analysis (state grouping + lock contention)
- [x] feat-004 In-browser result rendering
- [x] feat-007 Java code generation (lock-contention + deadlock reproducers)

### What's In Progress

- [ ] None — feat-007 complete.

### What's Next

1. feat-005 Export HTML/PDF (re-scoped: same css/js as app, PDF via pdf-lib one-page).
2. feat-009 ThreadMXBean lock-contention bug fix.
3. feat-010 Deadlock cycle detection (analyzer side).

## Blockers / Risks

- [ ] Risk: `pnpm -C web run build` requires `wasm-pack` + the `wasm32-unknown-unknown` target (installed via update script / one-time setup).

## Decisions Made

- **Pure-Rust parser split**: parsing lives in `src/parser.rs` (no wasm deps) so it is host-testable with `cargo test`; `src/lib.rs` only holds bindings.
- **feat-007 codegen**: `src/codegen.rs` emits two scenarios (lock-contention holder + BLOCKED waiters; deadlock via circular `synchronized` acquisition). Count clamped 2..=64. Exposed as `generateJava(scenario, count)` and a CLI `examples/gen_java.rs`.
- **init.sh ordering fix**: `pnpm -C web run wasm` now runs before typecheck/lint (typecheck needs the generated `web/src/wasm/*.d.ts`, which now includes `generateJava`).

## Files Modified This Session

- `src/codegen.rs` (new), `src/tests/java_code_generation.rs` (new), `src/lib.rs` (binding), `examples/gen_java.rs` (new)
- `web/src/analyzer.ts`, `web/src/App.tsx`, `web/src/index.css` (Generate Java panel)
- `feature_list.json`, `progress.md`, `init.sh`, `AGENTS.md`

## Evidence of Completion

- [x] Tests pass: `cargo test` -> 10 passed (5 codegen + 5 parser)
- [x] Lint/type/build clean: `./init.sh` green
- [x] javac 21: generated `DeadlockCycle.java` compiled + ran; SIGQUIT dump -> "Found one Java-level deadlock" (deadlock-0->1->2)
- [x] Browser: "Generate Java reproducer" panel renders code (see PR demo)

## Notes for Next Session

Run `./init.sh` first. Remember WASM is not hot-reloaded: re-run `pnpm -C web run wasm` after editing `src/*.rs`.
