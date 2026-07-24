//! Pure-Rust thread dump parsing and analysis.
//!
//! This module is intentionally free of any `wasm-bindgen` dependency so that
//! it can be unit-tested on the host target with `cargo test`.

use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Known `java.lang.Thread.State` values, ordered for stable display.
const KNOWN_STATES: [&str; 6] = [
    "NEW",
    "RUNNABLE",
    "BLOCKED",
    "WAITING",
    "TIMED_WAITING",
    "TERMINATED",
];

/// The thread dump text format that was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DumpFormat {
    /// Output of the `jstack` tool (state on its own `java.lang.Thread.State:` line).
    Jstack,
    /// Output of `ThreadMXBean#dumpAllThreads` / `ThreadInfo#toString` (state in the header line).
    ThreadMxBean,
    /// Could not be confidently classified; parsed on a best-effort basis.
    Unknown,
}

/// A single parsed thread.
#[derive(Debug, Clone, Serialize)]
pub struct ThreadInfo {
    pub name: String,
    pub id: Option<String>,
    pub state: String,
    /// Monitor/lock id this thread is trying to acquire (i.e. it is blocked on it).
    pub waiting_on: Option<String>,
    /// Monitor/lock ids currently held by this thread.
    pub held_locks: Vec<String>,
    /// Number of stack frames captured for this thread.
    pub stack_depth: usize,
}

/// Count of threads in a given state.
#[derive(Debug, Clone, Serialize)]
pub struct StateCount {
    pub state: String,
    pub count: usize,
}

/// A "thread A is blocked waiting for a lock held by thread B" relationship.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedEdge {
    pub blocked_thread: String,
    pub lock: String,
    pub owner_thread: Option<String>,
}

/// A detected deadlock: a set of threads in a circular wait-for relationship.
#[derive(Debug, Clone, Serialize)]
pub struct Deadlock {
    /// Thread names ordered around the cycle (each waits for the next).
    pub threads: Vec<String>,
    /// The wait-for edges that close the cycle (one per participant).
    pub edges: Vec<BlockedEdge>,
}

/// The full analysis result returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub format: DumpFormat,
    pub total_threads: usize,
    pub state_counts: Vec<StateCount>,
    pub threads: Vec<ThreadInfo>,
    /// Detected lock-contention edges (blocking problem pattern).
    pub blocked_edges: Vec<BlockedEdge>,
    /// Detected deadlock cycles (circular wait-for among threads).
    pub deadlocks: Vec<Deadlock>,
}

fn detect_format(input: &str) -> DumpFormat {
    if input.contains("java.lang.Thread.State:") {
        DumpFormat::Jstack
    } else if Regex::new(r#""[^"]*"\s+Id=\d+"#).unwrap().is_match(input) {
        DumpFormat::ThreadMxBean
    } else {
        DumpFormat::Unknown
    }
}

/// Decide whether a line begins a real thread stack entry.
///
/// Real headers look like `"name" #12 prio=5 ... tid=0x..` (jstack) or
/// `"name" Id=13 WAITING ..` (ThreadMXBean). This deliberately rejects the
/// `"deadlock-0":` lines from jstack's "Found one Java-level deadlock" summary
/// preamble, which would otherwise be mistaken for extra threads.
fn is_thread_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('"') {
        return false;
    }
    let rest = &trimmed[1..];
    let Some(end) = rest.find('"') else {
        return false;
    };
    let after = rest[end + 1..].trim_start();
    if after.is_empty() || after.starts_with(':') {
        // Bare quoted string or `"name":` summary line — not a stack header.
        return false;
    }
    after.starts_with('#')
        || after.starts_with("daemon")
        || after.contains("Id=")
        || after.contains("prio=")
        || after.contains("prio ")
        || after.contains("os_prio")
        || after.contains("tid=")
        || after.contains("nid=")
}

/// Split the dump into per-thread blocks. A new block starts on a recognized
/// thread header line; other lines attach to the current block.
fn split_thread_blocks(input: &str) -> Vec<Vec<&str>> {
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    for line in input.lines() {
        if is_thread_header(line) {
            blocks.push(vec![line]);
        } else if let Some(last) = blocks.last_mut() {
            last.push(line);
        }
    }
    blocks
}

fn extract_name(header: &str) -> Option<String> {
    let start = header.find('"')?;
    let rest = &header[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_block(block: &[&str], lock_re: &Regex, id_re: &Regex, state_re: &Regex) -> ThreadInfo {
    let header = block[0];
    let name = extract_name(header).unwrap_or_else(|| "<unknown>".to_string());

    let id = id_re
        .captures(header)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    // State: prefer the explicit jstack line, fall back to a token in the header.
    let mut state = String::new();
    for line in block {
        if let Some(idx) = line.find("java.lang.Thread.State:") {
            let after = &line[idx + "java.lang.Thread.State:".len()..];
            if let Some(tok) = after.split_whitespace().next() {
                state = tok.to_string();
            }
            break;
        }
    }
    if state.is_empty() {
        if let Some(c) = state_re.captures(header) {
            state = c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        }
    }
    if state.is_empty() {
        state = "UNKNOWN".to_string();
    }

    let mut waiting_on: Option<String> = None;
    let mut held_locks: Vec<String> = Vec::new();
    let mut stack_depth = 0usize;

    for line in &block[1..] {
        let trimmed = line.trim_start();
        if trimmed.starts_with("at ") {
            stack_depth += 1;
        }
        if let Some(cap) = lock_re.captures(line) {
            let lock_id = cap.get(1).map(|m| m.as_str().to_string());
            if let Some(lock_id) = lock_id {
                if trimmed.contains("waiting to lock") || trimmed.contains("- waiting to lock") {
                    waiting_on = Some(lock_id);
                } else if trimmed.contains("locked") {
                    held_locks.push(lock_id);
                }
            }
        }
    }

    ThreadInfo {
        name,
        id,
        state,
        waiting_on,
        held_locks,
        stack_depth,
    }
}

/// Parse and analyze a Java thread dump.
pub fn analyze(input: &str) -> Analysis {
    let format = detect_format(input);
    let lock_re = Regex::new(r"<(0x[0-9a-fA-F]+)>").unwrap();
    let id_re = Regex::new(r"Id=(\d+)").unwrap();
    let state_re = Regex::new(
        r"\b(NEW|RUNNABLE|BLOCKED|WAITING|TIMED_WAITING|TERMINATED)\b",
    )
    .unwrap();

    let threads: Vec<ThreadInfo> = split_thread_blocks(input)
        .iter()
        .map(|b| parse_block(b, &lock_re, &id_re, &state_re))
        .collect();

    // State grouping counts.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in &threads {
        *counts.entry(t.state.clone()).or_insert(0) += 1;
    }
    let mut state_counts: Vec<StateCount> = Vec::new();
    for s in KNOWN_STATES {
        if let Some(&c) = counts.get(s) {
            state_counts.push(StateCount {
                state: s.to_string(),
                count: c,
            });
        }
    }
    // Any non-standard states discovered afterwards.
    for (state, count) in &counts {
        if !KNOWN_STATES.contains(&state.as_str()) {
            state_counts.push(StateCount {
                state: state.clone(),
                count: *count,
            });
        }
    }

    // Lock-contention edges: map lock id -> owning thread name.
    let mut lock_owner: BTreeMap<String, String> = BTreeMap::new();
    for t in &threads {
        for lock in &t.held_locks {
            lock_owner
                .entry(lock.clone())
                .or_insert_with(|| t.name.clone());
        }
    }
    let mut blocked_edges: Vec<BlockedEdge> = Vec::new();
    for t in &threads {
        if let Some(lock) = &t.waiting_on {
            blocked_edges.push(BlockedEdge {
                blocked_thread: t.name.clone(),
                lock: lock.clone(),
                owner_thread: lock_owner.get(lock).cloned(),
            });
        }
    }

    let deadlocks = detect_deadlocks(&threads, &lock_owner);

    Analysis {
        format,
        total_threads: threads.len(),
        state_counts,
        threads,
        blocked_edges,
        deadlocks,
    }
}

/// Detect deadlock cycles from the wait-for graph.
///
/// Each thread waits on at most one lock, so the wait-for relation is a
/// functional graph (out-degree <= 1). We follow each chain; if it returns to a
/// node already on the current path, the tail from that node forms a cycle.
fn detect_deadlocks(
    threads: &[ThreadInfo],
    lock_owner: &BTreeMap<String, String>,
) -> Vec<Deadlock> {
    // wait_for[name] = (lock it waits on, owner thread of that lock)
    let mut wait_for: HashMap<&str, (&str, &str)> = HashMap::new();
    for t in threads {
        if let Some(lock) = &t.waiting_on {
            if let Some(owner) = lock_owner.get(lock) {
                if owner != &t.name {
                    wait_for.insert(t.name.as_str(), (lock.as_str(), owner.as_str()));
                }
            }
        }
    }

    // Deterministic iteration order over start nodes.
    let mut starts: Vec<&str> = wait_for.keys().copied().collect();
    starts.sort_unstable();

    let mut in_cycle: BTreeSet<String> = BTreeSet::new();
    let mut deadlocks: Vec<Deadlock> = Vec::new();

    for start in starts {
        if in_cycle.contains(start) {
            continue;
        }
        let mut path: Vec<&str> = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut cur = start;
        loop {
            if let Some(&pos) = index.get(cur) {
                let cycle: Vec<&str> = path[pos..].to_vec();
                if cycle.iter().all(|n| !in_cycle.contains(*n)) {
                    let mut edges = Vec::new();
                    for &name in &cycle {
                        let (lock, owner) = wait_for[name];
                        edges.push(BlockedEdge {
                            blocked_thread: name.to_string(),
                            lock: lock.to_string(),
                            owner_thread: Some(owner.to_string()),
                        });
                        in_cycle.insert(name.to_string());
                    }
                    deadlocks.push(Deadlock {
                        threads: cycle.iter().map(|s| s.to_string()).collect(),
                        edges,
                    });
                }
                break;
            }
            index.insert(cur, path.len());
            path.push(cur);
            match wait_for.get(cur) {
                Some(&(_, owner)) => cur = owner,
                None => break,
            }
        }
    }

    deadlocks
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSTACK_SAMPLE: &str = r#"2024-01-01 00:00:00
Full thread dump Java HotSpot(TM) 64-Bit Server VM:

"main" #1 prio=5 os_prio=0 tid=0x00007f0001 nid=0x1 waiting for monitor entry [0x00007f0002]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.run(App.java:10)
        - waiting to lock <0x000000076ab00000> (a java.lang.Object)
        - locked <0x000000076ab11111> (a java.lang.Object)

"worker" #2 prio=5 os_prio=0 tid=0x00007f0003 nid=0x2 runnable [0x00007f0004]
   java.lang.Thread.State: RUNNABLE
        at com.example.Worker.work(Worker.java:20)
        - locked <0x000000076ab00000> (a java.lang.Object)

"idle" #3 prio=5 os_prio=0 tid=0x00007f0005 nid=0x3 waiting on condition [0x00007f0006]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
"#;

    const MXBEAN_SAMPLE: &str = r#""main" Id=1 RUNNABLE
        at com.example.App.run(App.java:10)

"Thread-0" Id=13 WAITING on java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject@1234
        at sun.misc.Unsafe.park(Native Method)
"#;

    #[test]
    fn detects_jstack_format() {
        let a = analyze(JSTACK_SAMPLE);
        assert_eq!(a.format, DumpFormat::Jstack);
        assert_eq!(a.total_threads, 3);
    }

    #[test]
    fn detects_mxbean_format() {
        let a = analyze(MXBEAN_SAMPLE);
        assert_eq!(a.format, DumpFormat::ThreadMxBean);
        assert_eq!(a.total_threads, 2);
    }

    #[test]
    fn groups_states() {
        let a = analyze(JSTACK_SAMPLE);
        let get = |s: &str| a.state_counts.iter().find(|c| c.state == s).map(|c| c.count);
        assert_eq!(get("BLOCKED"), Some(1));
        assert_eq!(get("RUNNABLE"), Some(1));
        assert_eq!(get("TIMED_WAITING"), Some(1));
    }

    #[test]
    fn detects_lock_contention() {
        let a = analyze(JSTACK_SAMPLE);
        assert_eq!(a.blocked_edges.len(), 1);
        let edge = &a.blocked_edges[0];
        assert_eq!(edge.blocked_thread, "main");
        assert_eq!(edge.lock, "0x000000076ab00000");
        assert_eq!(edge.owner_thread.as_deref(), Some("worker"));
    }

    #[test]
    fn counts_stack_depth() {
        let a = analyze(JSTACK_SAMPLE);
        let main = a.threads.iter().find(|t| t.name == "main").unwrap();
        assert_eq!(main.stack_depth, 1);
        assert_eq!(main.held_locks, vec!["0x000000076ab11111"]);
    }

    // Simple contention (main blocked by worker) is NOT a deadlock: worker is
    // not itself blocked, so there is no cycle.
    #[test]
    fn no_false_deadlock_on_simple_contention() {
        let a = analyze(JSTACK_SAMPLE);
        assert!(a.deadlocks.is_empty());
    }

    const DEADLOCK_SAMPLE: &str = r#""t-A" #1 prio=5 os_prio=0 tid=0x0001 nid=0x1 waiting for monitor entry [0x1]
   java.lang.Thread.State: BLOCKED (on object monitor)
        - waiting to lock <0x000000000000000a> (a java.lang.Object)
        - locked <0x000000000000000b> (a java.lang.Object)

"t-B" #2 prio=5 os_prio=0 tid=0x0002 nid=0x2 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        - waiting to lock <0x000000000000000b> (a java.lang.Object)
        - locked <0x000000000000000a> (a java.lang.Object)
"#;

    #[test]
    fn detects_two_thread_deadlock() {
        let a = analyze(DEADLOCK_SAMPLE);
        assert_eq!(a.deadlocks.len(), 1);
        let dl = &a.deadlocks[0];
        assert_eq!(dl.threads.len(), 2);
        let members: BTreeSet<&str> = dl.threads.iter().map(|s| s.as_str()).collect();
        assert!(members.contains("t-A"));
        assert!(members.contains("t-B"));
        assert_eq!(dl.edges.len(), 2);
    }

    // feat-006: real jstack captured from a generated DeadlockCycle (javac 21),
    // including the "Found one Java-level deadlock" summary preamble.
    const REAL_JSTACK: &str = include_str!("../tests/fixtures/deadlock_real_jstack.txt");

    #[test]
    fn parses_real_world_deadlock_dump() {
        let a = analyze(REAL_JSTACK);
        assert_eq!(a.format, DumpFormat::Jstack);
        // The deadlock summary lines ("deadlock-0":) must NOT be parsed as threads.
        let phantom = a
            .threads
            .iter()
            .filter(|t| t.name.starts_with("deadlock-") && t.state == "UNKNOWN")
            .count();
        assert_eq!(phantom, 0, "summary lines leaked in as phantom threads");
        // Exactly one 3-thread deadlock cycle.
        assert_eq!(a.deadlocks.len(), 1);
        assert_eq!(a.deadlocks[0].threads.len(), 3);
        let members: BTreeSet<&str> =
            a.deadlocks[0].threads.iter().map(|s| s.as_str()).collect();
        for name in ["deadlock-0", "deadlock-1", "deadlock-2"] {
            assert!(members.contains(name), "missing {name} in cycle");
        }
    }
}
