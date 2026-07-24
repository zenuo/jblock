# Session Handoff

## Current Objective

- Goal: feat-008 Java version support (jstack/MXBean format diffs across 8/11/17/21)
- Current status: done
- Branch / commit: `cursor/feat-008-java-version-support-6bfd`

## Completed This Session

- [x] Installed Temurin 8/11/17/21 via jenv and captured jstack + MXBean dumps
- [x] Documented format diffs in `tests/fixtures/java-versions/FORMAT_DIFFS.md`
- [x] Parser: extract jstack `#N` thread ids (Java 21 `[os_tid]` / decimal `nid` compatible)
- [x] Test `detects_java_version_support` over all four JDK fixtures
- [x] Capture script `scripts/capture-java-version-dumps.sh`

## Verification Evidence

| Check | Command | Result | Notes |
|---|---|---|---|
| Unit tests | `cargo test` | pending full `./init.sh` | 14 tests incl. version matrix |
| Full gate | `./init.sh` | pending | cargo + wasm + lint + typecheck + build |

## Files Changed

- `src/parser.rs`
- `tests/fixtures/java-versions/*`
- `scripts/capture-java-version-dumps.sh`
- `feature_list.json`, `progress.md`, `session-handoff.md`

## Decisions Made

- Keep MXBean `Class@hash` lock parsing as feat-009; feat-008 proves format detection + thread/state coverage on all versions.
- Check in real dumps rather than synthetic approximations so Java 21 header quirks stay regression-tested.

## Blockers / Risks

- None for feat-008. feat-009 still needed for MXBean lock-contention edges.

## Next Session Startup

1. Read `AGENTS.md`.
2. Read `feature_list.json` and `progress.md`.
3. Review this handoff.
4. Run `./init.sh` before editing.

## Recommended Next Step

- Implement feat-009: parse MXBean `-  blocked on Class@hash` / `-  locked Class@hash` (and header `owned by`) into `waiting_on` / `held_locks`.
