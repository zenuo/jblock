#!/usr/bin/env bash
# Capture jstack / ThreadMXBean dumps across Java 8/11/17/21 (feat-008).
#
# Prerequisites:
#   - jenv with Temurin (or other) JDKs registered as: 1.8, 11, 17, 21
#   - jstack available on PATH for each selected JDK
#
# Usage (from repo root):
#   ./scripts/capture-java-version-dumps.sh
#
# Writes fixtures under tests/fixtures/java-versions/.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/tests/fixtures/java-versions"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$FIXTURES" "$WORK/src" "$WORK/out"

if ! command -v jenv >/dev/null 2>&1; then
  echo "jenv not found. Install https://github.com/jenv/jenv and add JDKs 8/11/17/21." >&2
  exit 1
fi

export PATH="${HOME}/.jenv/bin:${PATH}"
eval "$(jenv init -)"

cat > "$WORK/src/LockContention.java" <<'EOF'
public class LockContention {
    private static final Object LOCK = new Object();

    public static void main(String[] args) throws Exception {
        Thread holder = new Thread(() -> {
            synchronized (LOCK) {
                System.out.println("holder acquired LOCK");
                try { Thread.sleep(Long.MAX_VALUE); } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            }
        }, "holder");
        holder.start();
        Thread.sleep(300);
        for (int i = 0; i < 2; i++) {
            final int id = i;
            new Thread(() -> {
                synchronized (LOCK) {
                    System.out.println("waiter-" + id + " acquired");
                }
            }, "waiter-" + i).start();
        }
        Thread.sleep(Long.MAX_VALUE);
    }
}
EOF

cat > "$WORK/src/MxBeanDump.java" <<'EOF'
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadInfo;
import java.lang.management.ThreadMXBean;

public class MxBeanDump {
    private static final Object LOCK = new Object();

    public static void main(String[] args) throws Exception {
        Thread holder = new Thread(() -> {
            synchronized (LOCK) {
                try { Thread.sleep(Long.MAX_VALUE); } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            }
        }, "holder");
        holder.start();
        Thread.sleep(200);
        for (int i = 0; i < 2; i++) {
            final int id = i;
            new Thread(() -> {
                synchronized (LOCK) {
                    System.out.println("waiter-" + id);
                }
            }, "waiter-" + i).start();
        }
        Thread.sleep(500);

        ThreadMXBean bean = ManagementFactory.getThreadMXBean();
        ThreadInfo[] infos = bean.dumpAllThreads(
            bean.isObjectMonitorUsageSupported(),
            bean.isSynchronizerUsageSupported());
        for (ThreadInfo info : infos) {
            System.out.print(info.toString());
            System.out.println();
        }
        System.exit(0);
    }
}
EOF

cat > "$WORK/src/DeadlockCycle.java" <<'EOF'
public class DeadlockCycle {
    public static void main(String[] args) throws Exception {
        final int n = 3;
        final Object[] locks = new Object[n];
        for (int i = 0; i < n; i++) locks[i] = new Object();
        for (int i = 0; i < n; i++) {
            final int id = i;
            new Thread(() -> {
                synchronized (locks[id]) {
                    try { Thread.sleep(200); } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        return;
                    }
                    synchronized (locks[(id + 1) % n]) {
                        System.out.println("deadlock-" + id + " unexpected");
                    }
                }
            }, "deadlock-" + id).start();
        }
        Thread.sleep(Long.MAX_VALUE);
    }
}
EOF

for ver in 1.8 11 17 21; do
  label="$ver"
  if [ "$ver" = "1.8" ]; then label="8"; fi
  echo "=== Java $label ($ver) ==="
  JAVA_HOME="$(jenv prefix "$ver")"
  export JAVA_HOME
  export PATH="$JAVA_HOME/bin:$PATH"
  java -version 2>"$FIXTURES/java${label}-version.txt"

  out="$WORK/out/java${label}"
  mkdir -p "$out"
  javac -d "$out" "$WORK/src/LockContention.java" "$WORK/src/MxBeanDump.java" "$WORK/src/DeadlockCycle.java"

  java -cp "$out" LockContention >/dev/null 2>&1 &
  pid=$!
  sleep 1
  jstack "$pid" >"$FIXTURES/java${label}-jstack-contention.txt"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true

  java -cp "$out" DeadlockCycle >/dev/null 2>&1 &
  pid=$!
  sleep 1.5
  jstack "$pid" >"$FIXTURES/java${label}-jstack-deadlock.txt"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true

  java -cp "$out" MxBeanDump >"$FIXTURES/java${label}-mxbean-contention.txt"
done

echo "Wrote dumps to $FIXTURES"
