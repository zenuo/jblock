# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 07:20
**Active Feature:** feat-049 (done)

## Status

### What's Done

- [x] **feat-049** Virtual thread dump support (codegen-tested)
  - `DumpFormat::ThreadDumpJson` + `detect_format` for `jcmd Thread.dump_to_file -format=json`
  - `ThreadKind` (`platform` | `virtual` | `carrier`) + `carrier_id` / `mounted_id` on `ThreadInfo`; `web/src/types.ts` synced
  - `parse_thread_dump_json` + stack/container classification; jstack `Carrying`/`Mounted` linking via `link_jstack_virtual_carriers`
  - `Scenario::VirtualThreadBlock` codegen; `compile_run_dump_to_file_json` + `jdk21_tools_available` skip path
  - Offline fixtures under `tests/fixtures/virtual-threads/` (+ `JBLOCK_UPDATE_FIXTURES` refresh)
  - e2e `FEATURE_CHECKS["feat-049"]`

### What's In Progress

- [ ] (none — feature list complete through feat-049)

### What's Next

1. New features as added to `feature_list.json`

## Decisions Made

- JSON dump is primary VT source (jstack omits unmounted VTs; pinned VTs show only as Carrying on carriers)
- Missing mounted VT headers after Carrying are synthesized as `<virtual thread #N>` so carrier↔mounted links always exist
- State for JSON dumps is inferred from stack frames (JDK JSON omits Thread.State)
- `serde_json` added for WASM-capable JSON parsing

## Evidence of Completion

### Verification

```text
$ cargo test --lib
running 93 tests
… test result: ok. 93 passed; 0 failed; …

$ ./init.sh
=== cargo test === … 93 passed
=== pnpm lint / typecheck / build === ok
=== e2e feature matrix === Summary: 49/49 features PASS
```

### Files touched

- `Cargo.toml` / `Cargo.lock` — `serde_json`
- `src/parser.rs`, `src/codegen.rs`, `src/capture.rs`, `src/tests/java_code_generation.rs`
- `web/src/types.ts`
- `tests/fixtures/virtual-threads/`
- `scripts/e2e-features.mjs`, `feature_list.json`, `harness/e2e-results.json`
