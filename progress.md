# Session Progress Log

## Current State

**Last Updated:** 2026-09-02
**Active Feature:** feat-058 (done)

## Status

### What's Done

- [x] **feat-058** Remove unreliable Java version detection
  - Root cause: jstack header `Full thread dump … VM (25.45-b02 mixed mode)` is the **HotSpot VM** token for **Java 8u45**, not Java 25. `normalize_version_token` only mapped majors 23–25 to Java 6–8 when minor ≥ 100 (the Temurin 8 fixture is `25.492-b09`), so 8u0–8u99 displayed as `25.xx`.
  - Java 25 uses JEP 223 (`25` / `25.0.x+build`); that collides with HotSpot 25. MXBean dumps often have no version signal.
  - Withdrawn: `detect_java_version`, `Analysis.java_version`, Findings/HTML/CLI badges.
  - Kept: feat-055 green ✅ empty state; feat-008 format support across Java 8/11/17/21.

### What's In Progress

- [ ] (none)

### What's Next

1. (none for this batch)

## Decisions Made

- Do not guess a Java product version from dump text. A wrong badge is worse than no badge; HotSpot VM majors and JEP 223 cannot be disambiguated reliably across jstack / MXBean / JSON / other VMs.

## Evidence of Completion

Reproduced on user dump `ms82668.txt` (Java 8u45, 656 threads, jstack):

```text
# before
java_version= 25.45
format= jstack
total_threads= 656
```

Regression: `does_not_identify_hotspot_vm_token_as_java_product_version` (RED then GREEN).
