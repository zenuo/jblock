# Monitor contention live capture Implementation Plan

> **For agentic workers:** Implement task-by-task with TDD. Work on `main` (repo AGENTS.md).

**Goal:** Prove jblock’s waiter → monitor → owner edges from real JVM dumps produced by running generated Java (jstack + ThreadMXBean).

**Architecture:** Parser `blocked_edges` already maps `- waiting to lock` / `- blocked on` to the thread that `- locked` the same identity. The gap is a deterministic reproducer (`holder` then `waiter-*`) and committed dumps from a live JDK, matching feat-027’s capture harness.

**Tech Stack:** Rust `codegen` + `capture`, Java 21 `javac`/`java`/`jstack`/`ThreadMXBean`, React `web/src/codegen.ts` kept in sync.

**Global Constraints:**
- Do not invent a new `PatternKind`; assert `Analysis.blocked_edges`.
- Live tests skip when `jdk_tools_available()` is false.
- Write fixtures under `tests/fixtures/patterns/` (same as other capture tests).
- Keep WASM free of codegen/capture (`#[cfg(not(target_arch = "wasm32"))]`).

---

### Task 1: Deterministic LockContention Java

**Files:** `src/codegen.rs`, `web/src/codegen.ts`, `src/tests/java_code_generation.rs`

- [ ] Change generator to: named `holder` acquires `LOCK` first (`CountDownLatch`), then `waiter-i` threads `synchronized (LOCK)` and stay BLOCKED. Optional `-Djblock.mxbean.dump=<path>` writes `ThreadMXBean.dumpAllThreads(true, true)` after waiters start.
- [ ] Mirror the same source in `web/src/codegen.ts`.
- [ ] Update `lock_contention_has_expected_structure` to require `"holder"`, `"waiter-" + i`, `CountDownLatch`, not `worker-`.

### Task 2: Capture MXBean dump from the running JVM

**Files:** `src/capture.rs`

- [ ] Add `compile_run_mxbean_dump(source, class_name, warmup)` that compiles, runs with `-Djblock.mxbean.dump=…`, waits until the file contains `BLOCKED`, kills the process, returns the text.

### Task 3: Live tests + committed fixtures

**Files:** `src/capture.rs` tests, `tests/fixtures/patterns/lock_contention_jstack.txt`, `tests/fixtures/patterns/lock_contention_mxbean.txt`

- [ ] `live_capture_lock_contention_detects_edges`: `generate(LockContention, 3)`, `compile_run_jstack`, assert ≥2 edges, owner `holder`, waiters `waiter-*`, same lock id.
- [ ] `live_capture_lock_contention_mxbean_detects_edges`: same asserts on MXBean dump, `DumpFormat::ThreadMxBean`.
- [ ] Run with `JBLOCK_UPDATE_FIXTURES=1` and commit both dumps.

### Task 4: Offline parser tests + harness

**Files:** `src/parser.rs` tests, `feature_list.json` feat-060, `scripts/e2e-features.mjs`, `progress.md`

- [ ] `detects_lock_contention_from_live_fixture` / `detects_mxbean_lock_contention_from_live_fixture` via `include_str!`.
- [ ] e2e FEATURE_CHECKS for the new cargo tests + fixture files + `compile_run_mxbean_dump`.
- [ ] `./init.sh` green; commit and push `main`.
