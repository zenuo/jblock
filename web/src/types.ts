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

export interface Analysis {
  format: DumpFormat;
  total_threads: number;
  state_counts: StateCount[];
  threads: ThreadInfo[];
  blocked_edges: BlockedEdge[];
}
