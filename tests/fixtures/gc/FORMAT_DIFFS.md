# GC log fixtures

All `*.log` files here are **real JVM output** captured with
`scripts/capture-gc-logs.sh` (workload: `scripts/gc-capture/GcWorkload.java`).
Do not hand-edit them; re-run the script instead. Keep each file < 200KB.

| Fixture | JVM | Collector | `-Xlog` | What it exercises |
| --- | --- | --- | --- | --- |
| `g1-default-jdk21.log` | Temurin 21.0.12 | G1 | `gc*` (default decorators `uptime,level,tags`) | Baseline; `gc,init` facts; heap-at-exit noise |
| `g1-stress-jdk21.log` | Temurin 21.0.12 | G1, `-Xmx64m` | `gc*:…:time,uptime,level,tags` | Evacuation Failure, Full GC (G1 Compaction Pause / System.gc()), Mixed / Prepare Mixed, Concurrent Mark & Undo cycles, Remark/Cleanup sharing the cycle's `GC(n)` id |
| `g1-utctime-pid-jdk21.log` | Temurin 21.0.12 | G1 | `gc*:…:utctime,pid,tid,level,tags` | No uptime → wall-clock time axis; `pid` decorator |
| `g1-gc-only-jdk21.log` | Temurin 21.0.12 | G1 | `gc:…:uptimemillis` | Summary lines only (no tags decorator, no `gc,start` / `gc,heap`) |
| `parallel-jdk21.log` | Temurin 21.0.12 | Parallel | `gc*:…:time,uptime,level,tags` | `PSYoungGen` / `ParOldGen` with before/after capacities; System.gc() young + full pair |
| `serial-jdk21.log` | Temurin 21.0.12 | Serial | `gc*:…:uptimemillis,level,tags` | `DefNew` / `Tenured` |

## Unified logging notes (JDK 9+)

- Decorators are emitted in a fixed JVM order (time, utctime, uptime, timemillis,
  uptimemillis, timenanos, uptimenanos, hostname, pid, tid, level, tags) but any
  subset may be enabled, so the parser classifies each `[...]` by content.
- The tags column is space-padded and its width grows as longer tag sets appear
  (`[gc,start    ]` → `[gc,start       ]`).
- Heap / metaspace detail lines precede the summary line of the same `GC(n)`;
  the `gc,cpu` line follows it.
- G1 concurrent cycles share their `GC(n)` with the cycle's `Pause Remark` and
  `Pause Cleanup` (JDK 21).
- JDK 20+ prints `(Evacuation Failure)` in the summary; older JDKs log a separate
  `To-space exhausted` line. JDK 22+ may append a reason (`Evacuation Failure: Allocation`).
- `Pause Full (System.gc())` nests parentheses inside the cause.

## Rotated segments (feat-073)

| Fixture | `-Xlog` | Notes |
| --- | --- | --- |
| `g1-rotated-jdk21.log{.0,.1,.2,}` | `gc*:file=…::filecount=3,filesize=24k` (default decorators) | 4 segments, GC(0)–GC(104). `filecount` counts rotated files besides the current one. Only `.0` has `Using G1` / `gc,init`. GC(25), GC(57), GC(89) straddle boundaries. |
| `g1-rotated-wall-jdk21.log{.0,}` | `gc*:…:time,uptime,level,tags,pid:filecount=3,filesize=24k` | Wall + uptime + pid → JVM start instant identifies the JVM. |

- File names do not encode time order (`.0` is oldest here, the current file has no suffix, and numbers wrap once `filecount` is exceeded); order by line timestamps.

## JDK 8 `-XX:+PrintGCDetails` (feat-069)

Captured with Temurin 1.8.0_512 (`JAVA_HOME=$(mise where java@temurin-8) ./scripts/capture-gc-logs.sh`).

| Fixture | Collector | Flags beyond `PrintGCDetails` / date / time stamps | What it exercises |
| --- | --- | --- | --- |
| `jdk8-parallel.log` | Parallel | — | PSYoungGen / ParOldGen / Metaspace, System.gc() Full GC |
| `jdk8-parallel-refgc.log` | Parallel | `PrintGCCause`, `PrintReferenceGC`, `PrintTenuringDistribution` | Young GC split over 3 lines with inline `[SoftReference, …]` sub-phases and tenuring lines (production-style) |
| `jdk8-parallel-nostamps.log` | Parallel | only `PrintGCDetails` (`-Xloggc` still enables `PrintGCTimeStamps`) | uptime without wall clock |
| `jdk8-g1.log` | G1 | — | multi-line pauses with phase detail, `(young)` / `(mixed)` / `(initial-mark)`, `(to-space exhausted)`, remark / cleanup, `[GC concurrent-…]` lines |
| `jdk8-cms.log` | CMS | `CMSInitiatingOccupancyFraction=40` | ParNew, CMS Initial Mark / Final Remark, 7 concurrent phases, `ParNew (promotion failed)` + `[CMS: …]` fallback |
| `jdk8-serial.log` | Serial | — | DefNew / Tenured |
| `jdk8-parallel-rotated.log.{0,1}` | Parallel | `UseGCLogFileRotation`, `NumberOfGCLogFiles=3`, `GCLogFileSize=8K` | `GC log file has reached the maximum size. Saved as …` / `GC log file created …` markers; header repeated per file (the live `.N.current` file is renamed to `.N`) |

- Nested records repeat the date / uptime stamp before inner brackets (`[GC (…) 2026-…: 0.316: [ParNew: …]`); they are stripped before parsing.
- The record's duration is its outermost `, N secs]`; timed sub-phases without a size change (reference processing, remark steps) are removed first. G1 headers state the pause duration directly.
- A `[GC` record that also prints an old-generation bracket (`[CMS: …]`, `[Tenured: …]`) collected the old generation stop-the-world and is classified as a Full GC.
- Grep note: `[GC concurrent-…]` lines also start with `[GC`; exclude them when counting pauses.

## ZGC / Shenandoah (feat-070)

| Fixture | Flags | What it exercises |
| --- | --- | --- |
| `zgc-jdk21.log` | `-XX:+UseZGC -Xmx32m` | non-generational: `Garbage Collection (cause)` opens on `gc,start`, summary `18M(56%)->4M(12%)` without a duration; `Pause Mark Start / Mark End / Relocate Start` on `gc,phases`; `Allocation Stall (main) 1.158ms` without a `GC(n)` id; `Max Capacity` / `Initial Capacity` init keys; `Metaspace: 0M used, 0M committed, …` before the summary |
| `zgc-generational-jdk21.log` | `-XX:+ZGenerational -Xmx32m`, noisy tags off | `Minor` / `Major Collection (cause) 60M(94%)->12M(19%) 0.002s`; per-generation lines prefixed `y: ` / `Y: ` / `o: ` |
| `shenandoah-jdk21.log` | `-XX:+UseShenandoahGC -Xmx32m`, noisy tags off | `Pause Init Mark (unload classes)` (qualifier, not a cause), one `Pause Degenerated GC`, `Concurrent cleanup 8M->3M(32M)` carrying the heap |
| `shenandoah-passive-jdk21.log` | `-XX:ShenandoahGCMode=passive` | every cycle is `Pause Degenerated GC (Outside of Cycle) 45M->1M(48M)` |

- ZGC and Shenandoah report heap figures on cycles, not pauses: heap KPIs, allocation rate, GC09 / GC10 and the heap chart use every event that reports the heap.
- Allocation stalls are their own event kind (`stall`): not pauses, not concurrent cycles.

## Web sample

`web/src/sampleGc.log` (the **GC log sample** button) is a verbatim copy of `g1-stress-jdk21.log`; the e2e check for feat-081 keeps them identical.

## Gzip (feat-082)

`g1-rotated-jdk21.log.1.gz` is `gzip -n -9` of the real `g1-rotated-jdk21.log.1` segment; web (`DecompressionStream`) and CLI (`flate2`) decompress by magic bytes, not by file extension.
