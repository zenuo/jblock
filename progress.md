# Session Progress Log

## Current State

**Last Updated:** 2026-07-24 08:55
**Active Feature:** feat-008 (done)

## Status

### What's Done

- [x] feat-001 Project scaffold & verification (`./init.sh` green)
- [x] feat-002 Thread dump parsing (jstack + ThreadMXBean)
- [x] feat-003 Problem-pattern analysis (state grouping + lock contention)
- [x] feat-004 In-browser result rendering
- [x] feat-005 Format & UX hardening (drag-and-drop, deadlock detection, real-world coverage)
- [x] feat-006 Export HTML (app CSS) / PDF (pdf-lib one page)
- [x] feat-007 Java code generation (lock-contention + deadlock reproducers)
- [x] feat-008 Java version support (jenv 8/11/17/21 format diffs + fixtures + parser test)
- [x] feat-010 Deadlock cycle detection (done with feat-005)

### What's In Progress

- [ ] None — feat-008 complete.

### What's Next

1. feat-009 ThreadMXBean lock-contention bug fix (`Class@hash` locks).

## Blockers / Risks

- [ ] Risk: `pnpm -C web run build` requires `wasm-pack` + the `wasm32-unknown-unknown` target (installed via update script / one-time setup).
- [ ] Note: MXBean dumps from 8–21 parse format/threads/states, but lock edges still need feat-009 (`blocked on Class@hash` vs jstack `<0x…>`).

## Decisions Made

- **Pure-Rust parser split**: parsing lives in `src/parser.rs` (no wasm deps) so it is host-testable with `cargo test`; `src/lib.rs` only holds bindings.
- **feat-007 codegen**: `src/codegen.rs` emits two scenarios (lock-contention holder + BLOCKED waiters; deadlock via circular `synchronized` acquisition). Count clamped 2..=64. Exposed as `generateJava(scenario, count)` and a CLI `examples/gen_java.rs`.
- **feat-008 version matrix**: Temurin via jenv (`1.8`/`11`/`17`/`21`). Material diffs: jstack 11+ adds `cpu=`/`elapsed=`; jstack 21 adds `[os_tid]` and decimal `nid`; MXBean 11+ inserts `prio=` before `Id=` and uses module-prefixed frames; MXBean locks always use `Class@identityHash` (not `<0x…>`). Fixtures checked in; re-capture with `scripts/capture-java-version-dumps.sh`.
- **init.sh ordering fix**: `pnpm -C web run wasm` now runs before typecheck/lint (typecheck needs the generated `web/src/wasm/*.d.ts`, which now includes `generateJava`).

## Files Modified This Session (feat-008)

- `src/parser.rs` — jstack `#N` id extraction; `detects_java_version_support` test
- `tests/fixtures/java-versions/*` — real dumps + `FORMAT_DIFFS.md`
- `scripts/capture-java-version-dumps.sh` — reproducible capture via jenv
- `feature_list.json` / `progress.md` — mark feat-008 done

## Evidence of Completion

- [x] Tests pass: `cargo test` -> 14 passed (incl. `detects_java_version_support`)
- [x] Lint/type/build: `./init.sh` (run before claiming done)
- [x] Research: FORMAT_DIFFS.md compares jstack + MXBean across 8/11/17/21

## Notes for Next Session

Run `./init.sh` first. Remember WASM is not hot-reloaded: re-run `pnpm -C web run wasm` after editing `src/*.rs`. Next: feat-009 MXBean lock parsing.
