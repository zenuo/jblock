# Session Progress Log

## Current State

**Last Updated:** 2026-11-13
**Active Feature:** GC log analysis — feat-068 … feat-082 all done (recommended order completed)

## Status

### What's Done

- [x] Design `docs/design/gc-log-analysis.md` (now marked implemented, deviations recorded)
- [x] feat-068 unified-logging parser + real JDK 21 fixtures (`scripts/capture-gc-logs.sh`)
- [x] feat-071 stats engine (whole log / range; rate intervals < 50 ms merged for KPIs and charts)
- [x] feat-073 multi-file fingerprint / grouping / stitching (JVM start, pid, GC ids, JDK 8 rotation markers)
- [x] feat-074 input-kind detection, wasm `GcSession`, worker protocol, folder / drag-directory / append import, view switch
- [x] feat-075 GC page: uPlot pause timeline + heap chart, brush range → Rust range KPIs, uptime/wall axis
- [x] feat-072 findings GC01–GC14 (sourced thresholds) + findings panel with Locate
- [x] feat-076 histogram, causes, allocation/promotion rate, metaspace charts
- [x] feat-077 virtualized event table linked to charts + diagnostics
- [x] feat-069 JDK 8 PrintGCDetails parser (Parallel / G1 / CMS / Serial) + temurin-8 fixtures
- [x] feat-070 ZGC (incl. generational) / Shenandoah; heap KPIs from memory-reporting events; WASM size pass
- [x] feat-080 CLI GC support (`--kind`, `--grouping`, `--events`, causes/pauses/events sections)
- [x] feat-078 multi-JVM comparison (small multiples, worst badges, 8-group cap)
- [x] feat-079 standalone HTML export with ≤2000-point SVG charts
- [x] feat-081 GC log sample, home copy, help GC section, README
- [x] feat-082 `.gz` import (web DecompressionStream, CLI flate2)
- [x] CI fix: `web/.gitignore` 的 `*.log` 忽略了 `web/src/sampleGc.log`，Pages 构建报 `Could not resolve "./sampleGc.log?raw"`；加 `!src/sampleGc.log` 例外并提交。验证：干净 clone 中 `pnpm run build` 通过；`./init.sh` 82/82 PASS

### What's In Progress

- [ ] (none)

### What's Next

1. Commit the work in reviewable slices once git user.name / user.email are configured on this machine.
2. Optional: per-rule threshold tuning against more real production logs; safepoint / TTSP logs (out of scope v1).

### Decisions Made

- 2026-11-13 user confirmed: unified + JDK 8 formats; auto multi-file grouping with override; uPlot; one import entry; all four analysis areas; CLI + HTML export; design doc first; then "implement in the recommended order".
- Findings thresholds aligned to JMC; critical only for consecutive Full GC with <2% old-gen free (GCHeapFreeLimit semantics), heap free <2%, evacuation failure, CMS failures. Metadata GC Threshold counted after warm-up only (startup noise on the user's production logs).
- A file can be both thread dump and GC log; without any GC log in a batch, unknown texts still go to the thread analyzer (pre-074 behavior), in web and CLI.
- Chart colors: pause kinds use the 3 all-pairs-validated slots + neutral; heap/rates use violet (used/allocation) and green (old/promotion) by concept; magnitude bars use secondary ink.
- Comparison uses small multiples instead of an overlay (palette all-pairs limit, one meaning per color).
- WASM size: `opt-level = "z"` with regex-automata / aho-corasick / memchr at `"s"` (1366 → 1239 KiB; thread-dump analysis speed unchanged, GC parse ~60% slower).

### Unresolved Risks

- Nothing is committed: git user.name / user.email are not configured on this machine.
- Production build, synthetic 68 MB / 100k GCs: report 1.51 s, range 123 ms end-to-end (incl. redraw of ~300k points); the 50 ms stats-only budget was not measured in isolation.
- Browser verification scripts live in /tmp/jblock-browser (puppeteer-core + local Chrome), not in the repo gates.
- User-provided JDK 8 production logs at ~/Downloads/gc.log.0*.current are for local verification only (never commit).

## Evidence of Completion

```text
$ mise exec -- ./init.sh   # exit=0
cargo test --features cli: 186 passed; 0 failed
pnpm wasm / lint / typecheck / build: ok (dist wasm 1,269.71 kB = 1239 KiB; index js 584.65 kB / gzip 184.29 kB)
node scripts/e2e-features.mjs --skip-web: Summary: 82/82 features PASS
browser regression (local): check-069/070/072/074/075/076/077/078/079/081/082 all passed
local-only: user's two JDK 8u45 Parallel production logs → 8567 events = grep, 2 JVMs, GC02/GC08/GC10 critical + GC07 (web and CLI)
```

### Environment Notes

- Toolchain via mise global config (`~/.config/mise/config.toml`): rust stable (targets wasm32-unknown-unknown), `github:rustwasm/wasm-pack`, pnpm 10 (matches CI), java temurin-21 (+ temurin-8 installed for fixtures). Non-interactive shells need `mise exec -- ./init.sh`.

---

## Previous Session (2026-09-16)

**Last Updated:** 2026-09-16
**Active Feature:** feat-067 thread-state filter does not carry to a new analysis

## Status

### What's Done

- [x] Results 状态筛选默认 ALL，不再在有 BLOCKED 时自动筛 BLOCKED
- [x] 新 dump / 换分析时重置筛选；Results 按 dump digest remount
- [x] 点击 Thread states 仍只筛选当前这次分析

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- 浏览器 Clipboard API 在非安全上下文 / 未授权时会失败，已用粘贴模态框与 `paste` 事件兜底。
- CLI 剪贴板依赖本机工具；无 `pbpaste`/`wl-paste`/`xclip`/`xsel`/PowerShell 时 `-c` 会报错。

## Decisions Made

- Stayed on `main` per AGENTS.md.
- feat-015 “default emphasis on BLOCKED” 视为会串到新分析的 bug，改为每次分析从 All 开始。

## Evidence of Completion

```text
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
Summary: 67/67 features PASS
```

Browser: 当前页点 WAITING → Threads (1/17) 且 State=WAITING；新标签 Load sample → State ALL、Threads (12/17)，BLOCKED 未自动选中。
