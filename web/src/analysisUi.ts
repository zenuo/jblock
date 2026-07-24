import type { Analysis, BlockedEdge, ThreadInfo } from "./types";
import type { TranslateFn } from "./i18n";

export type FindingSeverity = "critical" | "warning" | "info";

export type FindingKind =
  | "deadlock"
  | "hot-lock"
  | "blocked"
  | "clean"
  | "thread-pool-exhaustion"
  | "sync-io-hotspot";

/** Actors shown in the pattern legend animation (feat-023). */
export interface FindingActor {
  /** Thread name from the dump. */
  thread: string;
  /** Top-of-stack Java class name when available. */
  className: string | null;
}

export interface FindingActors {
  /** Primary nodes (cycle members, sample healthy threads, or blocked threads). */
  nodes: FindingActor[];
  /** Lock owner when applicable. */
  owner: FindingActor | null;
  /** Waiters on the hottest / contended lock. */
  waiters: FindingActor[];
  /** Contended lock id (short form for display). */
  lock: string | null;
}

export interface Finding {
  severity: FindingSeverity;
  /** Pattern kind for legend/demo modal (feat-023). */
  kind: FindingKind;
  title: string;
  detail: string;
  /** Concrete dump actors for the legend animation. */
  actors: FindingActors;
}

/** Extract the Java class FQCN from a stack frame (`pkg.Cls.method(File:line)`). */
export function classNameFromFrame(frame: string): string | null {
  const trimmed = frame.trim().replace(/^at\s+/, "");
  if (!trimmed) return null;
  const beforeParen = trimmed.split("(")[0] ?? trimmed;
  const lastDot = beforeParen.lastIndexOf(".");
  if (lastDot <= 0) return beforeParen || null;
  return beforeParen.slice(0, lastDot);
}

function actorFor(
  analysis: Analysis,
  threadName: string | null | undefined,
): FindingActor | null {
  if (!threadName) return null;
  const th = analysis.threads.find((t) => t.name === threadName);
  const className = th?.stack[0] ? classNameFromFrame(th.stack[0]) : null;
  return { thread: threadName, className };
}

function actorsForNames(
  analysis: Analysis,
  names: string[],
  limit = 6,
): FindingActor[] {
  const out: FindingActor[] = [];
  for (const name of names) {
    const a = actorFor(analysis, name);
    if (a) out.push(a);
    if (out.length >= limit) break;
  }
  return out;
}

/** Truncate labels for SVG nodes. */
export function shortLabel(value: string, max = 14): string {
  if (value.length <= max) return value;
  return `${value.slice(0, Math.max(1, max - 1))}…`;
}

/** Prefer simple class name (`Foo` from `com.example.Foo`). */
export function shortClassName(className: string | null, max = 16): string {
  if (!className) return "";
  const simple = className.includes(".")
    ? className.slice(className.lastIndexOf(".") + 1)
    : className;
  return shortLabel(simple, max);
}

export interface ContentionGroup {
  lock: string;
  owner_thread: string | null;
  waiters: string[];
}

export interface StackCluster {
  signature: string;
  frames: string[];
  count: number;
  sample_names: string[];
  state: string;
}

/** feat-013: actionable findings for the top of the results page. */
export function buildFindings(
  analysis: Analysis,
  t: TranslateFn,
): Finding[] {
  const findings: Finding[] = [];
  const blocked =
    analysis.state_counts.find((s) => s.state === "BLOCKED")?.count ?? 0;
  const blockedPct =
    analysis.total_threads === 0
      ? 0
      : Math.round((blocked / analysis.total_threads) * 100);

  if (analysis.deadlocks.length > 0) {
    for (const d of analysis.deadlocks) {
      findings.push({
        severity: "critical",
        kind: "deadlock",
        title: t("findings.deadlockTitle", { count: d.threads.length }),
        detail: `${d.threads.join(" → ")} → ${d.threads[0] ?? ""}`,
        actors: {
          nodes: actorsForNames(analysis, d.threads, 8),
          owner: null,
          waiters: [],
          lock: d.edges[0]?.lock ?? null,
        },
      });
    }
  }

  for (const p of analysis.patterns ?? []) {
    if (p.kind === "thread-pool-exhaustion") {
      findings.push({
        severity: (p.severity as FindingSeverity) || "critical",
        kind: "thread-pool-exhaustion",
        title: t("findings.poolExhaustionTitle", {
          count: p.thread_names.length,
        }),
        detail: t("findings.poolExhaustionDetail", { detail: p.detail }),
        actors: {
          nodes: actorsForNames(analysis, p.thread_names, 6),
          owner: null,
          waiters: [],
          lock: null,
        },
      });
    } else if (p.kind === "sync-io-hotspot") {
      findings.push({
        severity: (p.severity as FindingSeverity) || "warning",
        kind: "sync-io-hotspot",
        title: t("findings.syncIoHotspotTitle", {
          count: p.thread_names.length,
        }),
        detail: t("findings.syncIoHotspotDetail", { detail: p.detail }),
        actors: {
          nodes: actorsForNames(analysis, p.thread_names, 6),
          owner: null,
          waiters: [],
          lock: null,
        },
      });
    }
  }

  const groups = aggregateContention(analysis.blocked_edges);
  if (groups.length > 0) {
    const hot = groups[0];
    findings.push({
      severity: analysis.deadlocks.length > 0 ? "warning" : "critical",
      kind: "hot-lock",
      title: t("findings.hotLockTitle", { count: hot.waiters.length }),
      detail: t("findings.hotLockDetail", {
        lock: hot.lock,
        owner: hot.owner_thread ?? t("deadlocks.unknown"),
      }),
      actors: {
        nodes: [],
        owner: actorFor(analysis, hot.owner_thread),
        waiters: actorsForNames(analysis, hot.waiters, 5),
        lock: hot.lock,
      },
    });
  }

  if (blocked > 0) {
    const blockedNames = analysis.blocked_edges.map((e) => e.blocked_thread);
    const firstOwner = analysis.blocked_edges.find(
      (e) => e.owner_thread,
    )?.owner_thread;
    const firstLock = analysis.blocked_edges[0]?.lock ?? null;
    findings.push({
      severity: blockedPct >= 20 ? "warning" : "info",
      kind: "blocked",
      title: t("findings.blockedTitle", { count: blocked, pct: blockedPct }),
      detail: t("findings.blockedDetail", {
        count: analysis.blocked_edges.length,
      }),
      actors: {
        nodes: actorsForNames(analysis, blockedNames, 5),
        owner: actorFor(analysis, firstOwner),
        waiters: [],
        lock: firstLock,
      },
    });
  } else if (
    analysis.deadlocks.length === 0 &&
    groups.length === 0 &&
    (analysis.patterns?.length ?? 0) === 0
  ) {
    const sample = analysis.threads
      .filter((th) => !isJvmNoise(th.name))
      .slice(0, 3)
      .map((th) => th.name);
    const fallback =
      sample.length > 0
        ? sample
        : analysis.threads.slice(0, 3).map((th) => th.name);
    findings.push({
      severity: "info",
      kind: "clean",
      title: t("findings.cleanTitle"),
      detail: t("findings.cleanDetail", {
        count: analysis.total_threads,
        format: analysis.format,
      }),
      actors: {
        nodes: actorsForNames(analysis, fallback, 3),
        owner: null,
        waiters: [],
        lock: null,
      },
    });
  }

  return findings;
}

/** feat-014: group edges by lock, hottest first. */
export function aggregateContention(edges: BlockedEdge[]): ContentionGroup[] {
  const map = new Map<string, ContentionGroup>();
  for (const e of edges) {
    let g = map.get(e.lock);
    if (!g) {
      g = { lock: e.lock, owner_thread: e.owner_thread, waiters: [] };
      map.set(e.lock, g);
    }
    if (!g.owner_thread && e.owner_thread) g.owner_thread = e.owner_thread;
    if (!g.waiters.includes(e.blocked_thread)) {
      g.waiters.push(e.blocked_thread);
    }
  }
  return [...map.values()].sort((a, b) => b.waiters.length - a.waiters.length);
}

/** feat-018: common HotSpot / JDK system thread names. */
export function isJvmNoise(name: string): boolean {
  const n = name.toLowerCase();
  const exact = [
    "reference handler",
    "finalizer",
    "signal dispatcher",
    "attach listener",
    "service thread",
    "common-cleaner",
    "notification thread",
    "monitor deflation thread",
    "vm thread",
    "vm periodic task thread",
    "destroyjavavm",
    "process reaper",
    "sweeper thread",
  ];
  if (exact.includes(n)) return true;
  return (
    n.startsWith("c1 compiler") ||
    n.startsWith("c2 compiler") ||
    n.startsWith("gc ") ||
    n.includes("gc thread") ||
    n.startsWith("g1 ") ||
    n.startsWith("gang worker") ||
    n.includes("parallelgc") ||
    n.startsWith("jvmci") ||
    n.includes("cleaner-")
  );
}

function stackSignature(frames: string[], depth = 5): string {
  return frames.slice(0, depth).join(" | ");
}

/** feat-019: cluster threads that share the same top frames. */
export function clusterByStack(
  threads: ThreadInfo[],
  minCount = 2,
): StackCluster[] {
  const map = new Map<string, StackCluster>();
  for (const t of threads) {
    if (t.stack.length === 0) continue;
    const frames = t.stack.slice(0, 5);
    const signature = stackSignature(frames);
    let c = map.get(signature);
    if (!c) {
      c = {
        signature,
        frames,
        count: 0,
        sample_names: [],
        state: t.state,
      };
      map.set(signature, c);
    }
    c.count += 1;
    if (c.sample_names.length < 5) c.sample_names.push(t.name);
  }
  return [...map.values()]
    .filter((c) => c.count >= minCount)
    .sort((a, b) => b.count - a.count);
}

export function threadKey(t: ThreadInfo, index: number): string {
  return `${t.name}::${t.id ?? index}`;
}

/** Safe DOM id for scrolling/highlighting a thread row. */
export function threadDomId(index: number): string {
  return `thread-row-${index}`;
}

export type ThreadSortKey = "name" | "state" | "stack" | "locks";

export function sortThreads(
  threads: ThreadInfo[],
  key: ThreadSortKey,
  dir: "asc" | "desc",
): ThreadInfo[] {
  const mul = dir === "asc" ? 1 : -1;
  return [...threads].sort((a, b) => {
    let cmp = 0;
    switch (key) {
      case "name":
        cmp = a.name.localeCompare(b.name);
        break;
      case "state":
        cmp = a.state.localeCompare(b.state) || a.name.localeCompare(b.name);
        break;
      case "stack":
        cmp = a.stack_depth - b.stack_depth;
        break;
      case "locks":
        cmp = a.held_locks.length - b.held_locks.length;
        break;
    }
    return cmp * mul;
  });
}
