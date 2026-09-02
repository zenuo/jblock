# Session Progress Log

## Current State

**Last Updated:** 2026-09-02
**Active Feature:** feat-058 (done)

## Status

### What's Done

- [x] **feat-058** Remove unreliable Java version detection
  - Root cause: jstack header `Full thread dump … VM (25.45-b02 mixed mode)` is the **HotSpot VM** token for **Java 8u45**, not Java 25. `normalize_version_token` only mapped majors 23–25 to Java 6–8 when minor ≥ 100 (Temurin 8 fixture is `25.492-b09`), so 8u0–8u99 displayed as `25.xx`.
  - Java 25 uses JEP 223 (`25` / `25.0.x+build`); that collides with HotSpot 25. MXBean dumps often have no version signal.
  - Withdrawn: `detect_java_version`, `Analysis.java_version`, Findings/HTML/CLI badges.
  - Kept: feat-055 green ✅ empty state; feat-008 format support across Java 8/11/17/21.

### What's In Progress

- [ ] (none)

### What's Next

1. (none for this batch)

## Decisions Made

- Do not guess a Java product version from dump text. A wrong badge is worse than no badge; HotSpot VM majors and JEP 223 cannot be disambiguated reliably across jstack / MXBean / JSON / other VMs.

## Verification Evidence

User dump `ms82668.txt` (Java 8u45, 656 threads) **before**: `java_version=25.45`.

```text
$ ./init.sh
cargo test --features cli  → 105 passed
pnpm -C web run wasm/lint/typecheck/build → ok
node scripts/e2e-features.mjs --skip-web → Summary: 58/58 features PASS (incl. feat-058)

$ cargo run --features cli --bin jblock -- -j ms82668.txt
# JSON keys: blocked_edges, deadlocks, format, patterns, state_counts, threads, total_threads
# java_version absent; format=jstack; total_threads=656
# text header: "jblock · jstack · 656 threads" (no Java 25.45)
```

Browser: sample Findings meta `17 个线程 · jstack`; ms82668 Findings meta `656 个线程 · jstack`; no Java version badge.
