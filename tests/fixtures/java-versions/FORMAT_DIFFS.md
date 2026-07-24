# Java 8 / 11 / 17 / 21 thread-dump format comparison

Captured with Temurin JDKs via [jenv](https://github.com/jenv/jenv) on 2026-07-24
using `scripts/capture-java-version-dumps.sh`.

Sample programs: lock-contention (`holder` + `waiter-*`) and a 3-thread
`DeadlockCycle`. Each JDK produced:

| File | Source |
| --- | --- |
| `java{N}-jstack-contention.txt` | `jstack <pid>` while contended |
| `java{N}-jstack-deadlock.txt` | `jstack <pid>` during circular wait |
| `java{N}-mxbean-contention.txt` | `ThreadMXBean#dumpAllThreads(true, true)` |
| `java{N}-version.txt` | `java -version` |

## jstack (`Full thread dump …`)

Shared across all versions:

- Thread header starts with `"name" #N …`
- State on its own line: `java.lang.Thread.State: BLOCKED|RUNNABLE|…`
- Monitor lines: `- waiting to lock <0x…>` / `- locked <0x…>`
- Deadlock summary: `Found one Java-level deadlock:` then `"name":` replay
  blocks (colon after the name — **not** real thread headers)

| Version | Header extras | Notes |
| --- | --- | --- |
| 8 | `prio=` `os_prio=` `tid=0x…` `nid=0x…` | Classic HotSpot layout |
| 11 | + `cpu=` `elapsed=` | Also `Threads class SMR info` preamble |
| 17 | same as 11 | Adds threads like `Monitor Deflation Thread` |
| 21 | + `[os_thread_id]` after `#N`; `nid` is **decimal** (no `0x`) | e.g. `"holder" #19 [6445] … nid=6445` |

Parser impact: header detection must accept `#N`, optional `[id]`, `cpu=` /
`elapsed=`, and decimal `nid`. Lock identity remains `<0x…>` on every version.

## ThreadMXBean (`ThreadInfo#toString`)

Shared across all versions:

- Header contains `Id=<n>` and the state token on the **same** line
- Lock identities use `Class@identityHash` (no angle brackets / no `0x` prefix)
- Waiter body: `-  blocked on java.lang.Object@…`
- Holder body: `-  locked java.lang.Object@…`
- Header may also say `BLOCKED on … owned by "holder" Id=…`

| Version | Header shape | Stack frames |
| --- | --- | --- |
| 8 | `"name" Id=N STATE …` | `java.lang.…` / `sun.…` |
| 11 | `"name" prio=P Id=N STATE …` | `java.base@11…/…` module prefix; `app//` for app code |
| 17 | same as 11 | `java.base@17…` |
| 21 | same as 11 | `Thread.sleep0` + `Thread.runWith`; lambda names `$$Lambda/0x…` |

Parser impact: format detection via `Id=`; jstack-style `<0x…>` lock regex does
**not** match MXBean locks (tracked as feat-009). Name / state / thread split
already work on 8–21.

## Compatibility matrix (feat-008)

| Dump | Detect format | Split threads | States | jstack contention | jstack deadlock | MXBean contention locks |
| --- | --- | --- | --- | --- | --- | --- |
| jstack 8/11/17/21 | yes | yes | yes | yes | yes | n/a |
| MXBean 8/11/17/21 | yes | yes | yes | n/a | n/a | no (feat-009) |
