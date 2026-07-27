// Mirror of the Rust `Analysis` struct serialized from the WASM module.
// Keep this in sync with `src/parser.rs`.

export type DumpFormat = "jstack" | "thread-mx-bean" | "unknown";

export interface ThreadInfo {
  name: string;
  id: string | null;
  state: string;
  waiting_on: string | null;
  held_locks: string[];
  stack_depth: number;
  /** Full stack frames without the leading `at ` (feat-046). */
  stack: string[];
}

export interface StateCount {
  state: string;
  count: number;
}

export interface BlockedEdge {
  blocked_thread: string;
  lock: string;
  owner_thread: string | null;
}

export interface Deadlock {
  threads: string[];
  edges: BlockedEdge[];
}

export type PatternKind =
  | "thread-pool-exhaustion"
  | "sync-io-hotspot"
  | "dangerous-hot-lock-owner"
  | "connection-pool-borrow"
  | "future-latch-wait-tree"
  | "logging-appender-contention"
  | "busy-wait-spin-hotspot"
  | "condition-park-starvation"
  | "lock-order-inconsistency"
  | "finalizer-pressure"
  | "sleep-as-scheduler"
  | "framework-pool-saturation"
  | "dns-resolution-stall"
  | "thread-leak"
  | "livelock";

export interface PatternHit {
  kind: PatternKind;
  severity: string;
  thread_names: string[];
  detail: string;
}

export interface Analysis {
  format: DumpFormat;
  total_threads: number;
  state_counts: StateCount[];
  threads: ThreadInfo[];
  blocked_edges: BlockedEdge[];
  deadlocks: Deadlock[];
  patterns: PatternHit[];
}

/** Ordered multi-dump analysis with cross-dump patterns (feat-041). */
export interface MultiDumpAnalysis {
  dumps: Analysis[];
  cross_patterns: PatternHit[];
}
