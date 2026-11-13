#!/usr/bin/env bash
# Capture real GC logs from a live JVM into tests/fixtures/gc/ (feat-068).
#
# Uses `java`/`javac` from JAVA_HOME (if set) or PATH. Each scenario runs
# scripts/gc-capture/GcWorkload.java with a specific collector and -Xlog
# decorator set so the parser is tested against genuine JVM output.
#
# Usage (from repo root):
#   ./scripts/capture-gc-logs.sh            # unified-log scenarios (JDK 9+ java)
#   ./scripts/capture-gc-logs.sh g1-stress  # one scenario by name
#   JAVA_HOME=$(mise where java@temurin-8) ./scripts/capture-gc-logs.sh
#                                           # JDK 8 -XX:+PrintGCDetails scenarios (feat-069)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/tests/fixtures/gc"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

JAVA_BIN="${JAVA_HOME:+$JAVA_HOME/bin/}java"
JAVAC_BIN="${JAVA_HOME:+$JAVA_HOME/bin/}javac"
FEATURE="$("$JAVA_BIN" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')"

mkdir -p "$OUT"
"$JAVAC_BIN" -source 8 -target 8 -d "$WORK" "$ROOT/scripts/gc-capture/GcWorkload.java" 2>/dev/null

# JDK 8 (-Xloggc + PrintGCDetails). Rotation writes %LOG%.0, %LOG%.1 … (current: .N.current).
LEGACY_DETAILS="-XX:+PrintGCDetails -XX:+PrintGCDateStamps -XX:+PrintGCTimeStamps"
LEGACY_SCENARIOS=(
  "parallel|-XX:+UseParallelGC -Xmx64m $LEGACY_DETAILS|10|churn,humongous,systemgc,leak|-Djblock.leakMBps=3"
  "parallel-refgc|-XX:+UseParallelGC -Xmx48m $LEGACY_DETAILS -XX:+PrintGCCause -XX:+PrintReferenceGC -XX:+PrintTenuringDistribution|12|churn,leak|-Djblock.leakMBps=3"
  "parallel-nostamps|-XX:+UseParallelGC -Xmx64m -XX:+PrintGCDetails|6|churn,systemgc|"
  "g1|-XX:+UseG1GC -Xmx64m $LEGACY_DETAILS|10|churn,humongous,systemgc,leak|-Djblock.leakMBps=4"
  "cms|-XX:+UseConcMarkSweepGC -Xmx48m -XX:CMSInitiatingOccupancyFraction=40 -XX:+UseCMSInitiatingOccupancyOnly $LEGACY_DETAILS|14|churn,leak|-Djblock.leakMBps=3 -Djblock.allocMBps=80"
  "serial|-XX:+UseSerialGC -Xmx64m $LEGACY_DETAILS|8|churn,systemgc,leak|-Djblock.leakMBps=3"
  "parallel-rotated|-XX:+UseParallelGC -Xmx64m $LEGACY_DETAILS -XX:+UseGCLogFileRotation -XX:NumberOfGCLogFiles=3 -XX:GCLogFileSize=8K|14|churn,systemgc,leak|-Djblock.leakMBps=2"
)

if [[ "$FEATURE" == "1.8" ]]; then
  only="${1:-}"
  for row in "${LEGACY_SCENARIOS[@]}"; do
    IFS='|' read -r name flags seconds modes props <<<"$row"
    if [[ -n "$only" && "$only" != "$name" ]]; then continue; fi
    log="$OUT/jdk8-${name}.log"
    rm -f "$log" "$log".[0-9]*
    echo "== $name -> ${log#$ROOT/}"
    # shellcheck disable=SC2086
    "$JAVA_BIN" $flags "-Xloggc:$log" $props -cp "$WORK" GcWorkload "$seconds" "$modes" >/dev/null 2>&1 || true
    # Rotation leaves the live file as <name>.N.current; keep it as <name>.N.
    for f in "$log".*.current; do [[ -f "$f" ]] && mv "$f" "${f%.current}"; done
    for f in "$log" "$log".[0-9]*; do
      [[ -f "$f" ]] && printf '   %s: %s lines, %s bytes\n' "${f##*/}" "$(wc -l <"$f")" "$(wc -c <"$f")"
    done
  done
  exit 0
fi

# name | jvm flags | xlog selector+decorators | seconds | modes | extra -D props
SCENARIOS=(
  "g1-default|-XX:+UseG1GC -Xmx128m|gc*:file=%LOG%|8|churn,humongous|-Djblock.allocMBps=60"
  "g1-stress|-XX:+UseG1GC -Xmx64m|gc*:file=%LOG%:time,uptime,level,tags|15|churn,humongous,systemgc,leak|-Djblock.leakMBps=3"
  "g1-utctime-pid|-XX:+UseG1GC -Xmx96m|gc*:file=%LOG%:utctime,pid,tid,level,tags|6|churn,leak|-Djblock.leakMBps=2"
  "g1-gc-only|-XX:+UseG1GC -Xmx64m|gc:file=%LOG%:uptimemillis|8|churn,humongous,systemgc|"
  "parallel|-XX:+UseParallelGC -Xmx96m|gc*:file=%LOG%:time,uptime,level,tags|10|churn,systemgc,leak|-Djblock.leakMBps=3"
  "serial|-XX:+UseSerialGC -Xmx96m|gc*:file=%LOG%:uptimemillis,level,tags|10|churn,leak|-Djblock.leakMBps=3"
  # Concurrent collectors (feat-070): small heap + fast allocation to provoke stalls / degenerated cycles.
  "zgc|-XX:+UseZGC -Xmx32m|gc*:file=%LOG%:time,uptime,level,tags|3|churn,humongous,systemgc,leak|-Djblock.leakMBps=4 -Djblock.allocMBps=150"
  "zgc-generational|-XX:+UseZGC -XX:+ZGenerational -Xmx32m|gc*,gc+task=off,gc+heap=off,gc+reloc=off,gc+alloc=off:file=%LOG%:time,uptime,level,tags|3|churn,humongous,systemgc,leak|-Djblock.leakMBps=4 -Djblock.allocMBps=150"
  "shenandoah|-XX:+UseShenandoahGC -Xmx32m|gc*,gc+task=off,gc+ergo=off,gc+stats=off,gc+ref=off,gc+marking=off:file=%LOG%:time,uptime,level,tags|3|churn,humongous,systemgc,leak|-Djblock.leakMBps=6 -Djblock.allocMBps=250"
  # Passive mode only runs STW degenerated / full cycles: real "Pause Degenerated GC" output.
  "shenandoah-passive|-XX:+UseShenandoahGC -XX:+UnlockDiagnosticVMOptions -XX:+UnlockExperimentalVMOptions -XX:ShenandoahGCMode=passive -Xmx48m|gc*,gc+task=off,gc+ergo=off,gc+stats=off,gc+ref=off,gc+marking=off:file=%LOG%:time,uptime,level,tags|3|churn,humongous,leak|-Djblock.leakMBps=4 -Djblock.allocMBps=150"
  # Rotated segments (feat-073): one JVM split into %LOG%, %LOG%.0, %LOG%.1 …
  "g1-rotated|-XX:+UseG1GC -Xmx64m|gc*:file=%LOG%::filecount=3,filesize=24k|14|churn,humongous,leak|-Djblock.leakMBps=1"
  "g1-rotated-wall|-XX:+UseG1GC -Xmx64m|gc*:file=%LOG%:time,uptime,level,tags,pid:filecount=3,filesize=24k|6|churn,humongous,leak|-Djblock.leakMBps=1"
)

only="${1:-}"
for row in "${SCENARIOS[@]}"; do
  IFS='|' read -r name flags xlog seconds modes props <<<"$row"
  if [[ -n "$only" && "$only" != "$name" ]]; then continue; fi
  log="$OUT/${name}-jdk${FEATURE}.log"
  rm -f "$log" "$log".[0-9]*
  echo "== $name -> ${log#$ROOT/}"
  # shellcheck disable=SC2086
  "$JAVA_BIN" $flags "-Xlog:${xlog//%LOG%/$log}" $props -cp "$WORK" GcWorkload "$seconds" "$modes" >/dev/null
  for f in "$log" "$log".[0-9]*; do
    [[ -f "$f" ]] && printf '   %s: %s lines, %s bytes\n' "${f##*/}" "$(wc -l <"$f")" "$(wc -c <"$f")"
  done
done
