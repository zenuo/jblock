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
    /// Top stack frames (`at …` lines without the leading `at `), capped for size.
    pub stack: Vec<String>,
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

/// Higher-level problem patterns beyond raw edges/cycles (feat-027+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PatternKind {
    /// Executor pool workers all busy/blocked; none idle in getTask.
    ThreadPoolExhaustion,
    /// Many threads share the same sync socket/HTTP/JDBC stack (feat-029).
    SyncIoHotspot,
    /// Hottest lock owner is blocked (sleep/I/O/park) while waiters pile up (feat-030).
    DangerousHotLockOwner,
    /// Many threads blocked borrowing from a connection pool (feat-031).
    ConnectionPoolBorrow,
    /// Wait tree on Future.get / CountDownLatch / CyclicBarrier (feat-032).
    FutureLatchWaitTree,
    /// Contended Log4j/Logback-style appender lock (feat-033).
    LoggingAppenderContention,
    /// Many RUNNABLE threads share a tight spin/busy-wait stack (feat-034).
    BusyWaitSpinHotspot,
    /// Many WAITING on ConditionObject/park with no signaler RUNNABLE (feat-035).
    ConditionParkStarvation,
    /// Conflicting lock acquisition orders that risk deadlock (feat-036).
    LockOrderInconsistency,
    /// Finalizer / Reference Handler busy or blocked under pressure (feat-037).
    FinalizerPressure,
    /// Business threads dominated by TIMED_WAITING Thread.sleep (feat-038).
    SleepAsScheduler,
    /// Tomcat/Jetty/Netty worker threads saturated on the same blocking work (feat-039).
    FrameworkPoolSaturation,
    /// Clusters stuck in InetAddress / DNS Resolver frames (feat-040).
    DnsResolutionStall,
    /// App thread count grows across dumps (feat-041).
    ThreadLeak,
    /// Same threads keep changing stacks across dumps without settling (feat-041).
    Livelock,
}

/// One detected high-level pattern hit.
#[derive(Debug, Clone, Serialize)]
pub struct PatternHit {
    pub kind: PatternKind,
    /// `critical` | `warning` | `info`
    pub severity: String,
    /// Threads involved in the pattern (sample / all pool workers).
    pub thread_names: Vec<String>,
    /// Machine-readable evidence for tests and exports.
    pub detail: String,
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
    /// Higher-level patterns (pool exhaustion, I/O hotspots, …).
    pub patterns: Vec<PatternHit>,
}

/// Result of analyzing an ordered series of dumps (feat-041).
#[derive(Debug, Clone, Serialize)]
pub struct MultiDumpAnalysis {
    /// Per-dump analysis in the same order as the inputs.
    pub dumps: Vec<Analysis>,
    /// Cross-dump-only patterns (thread leak, livelock).
    pub cross_patterns: Vec<PatternHit>,
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

/// Thread id from an MXBean `Id=N` header, or the jstack `#N` ordinal.
fn extract_id(header: &str, mxbean_id_re: &Regex, jstack_id_re: &Regex) -> Option<String> {
    mxbean_id_re
        .captures(header)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            jstack_id_re
                .captures(header)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        })
}

/// Max stack frames retained per thread (full depth still counted in `stack_depth`).
const MAX_STACK_FRAMES: usize = 12;

fn parse_block(
    block: &[&str],
    // jstack: `- waiting to lock <0x…>` / `- locked <0x…>`
    jstack_lock_re: &Regex,
    // MXBean: `-  blocked on Class@hash` / `-  locked Class@hash` / `-  waiting to lock Class@hash`
    mxbean_lock_re: &Regex,
    // Header fallback: `BLOCKED on Class@hash owned by "owner"`
    mxbean_header_blocked_re: &Regex,
    mxbean_id_re: &Regex,
    jstack_id_re: &Regex,
    state_re: &Regex,
) -> ThreadInfo {
    let header = block[0];
    let name = extract_name(header).unwrap_or_else(|| "<unknown>".to_string());

    let id = extract_id(header, mxbean_id_re, jstack_id_re);

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
    let mut stack: Vec<String> = Vec::new();

    for line in &block[1..] {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("at ") {
            stack_depth += 1;
            if stack.len() < MAX_STACK_FRAMES {
                stack.push(rest.to_string());
            }
        }
        // jstack monitor lines use angle-bracket hex identities.
        if let Some(cap) = jstack_lock_re.captures(line) {
            let lock_id = cap[1].to_string();
            if trimmed.contains("waiting to lock") {
                waiting_on = Some(lock_id);
            } else if trimmed.contains("locked") {
                held_locks.push(lock_id);
            }
            // Note: `- waiting on <0x…>` is Object.wait / park, not lock acquisition.
        }
        // ThreadMXBean / ThreadInfo#toString uses `Class@identityHash`.
        if let Some(cap) = mxbean_lock_re.captures(trimmed) {
            let kind = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let lock_id = cap[2].to_string();
            match kind {
                "blocked on" | "waiting to lock" => waiting_on = Some(lock_id),
                "locked" => held_locks.push(lock_id),
                _ => {}
            }
        }
    }

    // Header fallback when lockedMonitors details are missing but the
    // ThreadInfo header still carries `BLOCKED on Class@hash`.
    if waiting_on.is_none() {
        if let Some(cap) = mxbean_header_blocked_re.captures(header) {
            waiting_on = Some(cap[1].to_string());
        }
    }

    ThreadInfo {
        name,
        id,
        state,
        waiting_on,
        held_locks,
        stack_depth,
        stack,
    }
}

/// Parse and analyze a Java thread dump.
pub fn analyze(input: &str) -> Analysis {
    let format = detect_format(input);
    let jstack_lock_re = Regex::new(r"<(0x[0-9a-fA-F]+)>").unwrap();
    // MXBean lock lines: kind in group 1, `Class@hash` in group 2.
    // Do not match `-  waiting on …` (Condition/park), only acquisition/contention.
    let mxbean_lock_re = Regex::new(
        r"-\s+(blocked on|waiting to lock|locked)\s+([\w.$]+@[0-9a-fA-F]+)",
    )
    .unwrap();
    let mxbean_header_blocked_re =
        Regex::new(r"\bBLOCKED on ([\w.$]+@[0-9a-fA-F]+)").unwrap();
    // Optional owner name from the same header (`owned by "name"`).
    let mxbean_owned_by_re = Regex::new(r#"\bowned by "([^"]+)""#).unwrap();
    // MXBean headers: `"name" Id=13 …` (Java 8) or `"name" prio=5 Id=13 …` (11+).
    let mxbean_id_re = Regex::new(r"\bId=(\d+)").unwrap();
    // jstack headers: `"name" #19 …` (Java 21 may insert `[os_tid]` after `#N`).
    let jstack_id_re = Regex::new(r#"^"[^"]*"\s+#(\d+)"#).unwrap();
    let state_re = Regex::new(
        r"\b(NEW|RUNNABLE|BLOCKED|WAITING|TIMED_WAITING|TERMINATED)\b",
    )
    .unwrap();

    let blocks = split_thread_blocks(input);
    let threads: Vec<ThreadInfo> = blocks
        .iter()
        .map(|b| {
            parse_block(
                b,
                &jstack_lock_re,
                &mxbean_lock_re,
                &mxbean_header_blocked_re,
                &mxbean_id_re,
                &jstack_id_re,
                &state_re,
            )
        })
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
    // Harvest `owned by "…"` from MXBean BLOCKED headers when the owner's
    // `-  locked` line is absent (still enough to attribute the edge).
    for (t, block) in threads.iter().zip(blocks.iter()) {
        if let Some(lock) = &t.waiting_on {
            if lock_owner.contains_key(lock) {
                continue;
            }
            if let Some(cap) = mxbean_owned_by_re.captures(block[0]) {
                lock_owner.insert(lock.clone(), cap[1].to_string());
            }
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
    let patterns = detect_patterns(&threads);

    Analysis {
        format,
        total_threads: threads.len(),
        state_counts,
        threads,
        blocked_edges,
        deadlocks,
        patterns,
    }
}

/// Analyze an ordered series of dumps and detect cross-dump patterns (feat-041).
pub fn analyze_series(inputs: &[&str]) -> MultiDumpAnalysis {
    let dumps: Vec<Analysis> = inputs.iter().map(|s| analyze(s)).collect();
    let mut cross_patterns = Vec::new();
    if let Some(hit) = detect_thread_leak(&dumps) {
        cross_patterns.push(hit);
    }
    if let Some(hit) = detect_livelock(&dumps) {
        cross_patterns.push(hit);
    }
    MultiDumpAnalysis {
        dumps,
        cross_patterns,
    }
}

/// Detect higher-level patterns from parsed threads (feat-027+).
fn detect_patterns(threads: &[ThreadInfo]) -> Vec<PatternHit> {
    let mut out = Vec::new();
    if let Some(hit) = detect_thread_pool_exhaustion(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_sync_io_hotspot(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_dangerous_hot_lock_owner(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_connection_pool_borrow(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_future_latch_wait_tree(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_logging_appender_contention(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_busy_wait_spin_hotspot(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_condition_park_starvation(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_lock_order_inconsistency(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_finalizer_pressure(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_sleep_as_scheduler(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_framework_pool_saturation(threads) {
        out.push(hit);
    }
    if let Some(hit) = detect_dns_resolution_stall(threads) {
        out.push(hit);
    }
    out
}

fn is_executor_pool_thread(name: &str) -> bool {
    // Default Executors.newFixedThreadPool naming: pool-1-thread-1
    let re = Regex::new(r"^pool-\d+-thread-\d+$").unwrap();
    re.is_match(name)
}

fn stack_has_idle_get_task(stack: &[String]) -> bool {
    stack
        .iter()
        .any(|f| f.contains("ThreadPoolExecutor.getTask"))
}

/// Pool exhaustion: ≥3 `pool-N-thread-M` workers, none idle in getTask,
/// and at least one is BLOCKED or holding work (not purely idle).
fn detect_thread_pool_exhaustion(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let pool: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| is_executor_pool_thread(&t.name))
        .collect();
    if pool.len() < 3 {
        return None;
    }
    if pool.iter().any(|t| stack_has_idle_get_task(&t.stack)) {
        return None;
    }
    let busy = pool
        .iter()
        .filter(|t| {
            t.state == "BLOCKED"
                || t.state == "RUNNABLE"
                || t.state == "TIMED_WAITING"
                || t.state == "WAITING"
        })
        .count();
    if busy < 3 {
        return None;
    }
    // Prefer signal when some are BLOCKED (classic stuck-on-shared-lock exhaustion).
    let blocked = pool.iter().filter(|t| t.state == "BLOCKED").count();
    let severity = if blocked >= 2 {
        "critical"
    } else {
        "warning"
    };
    let names: Vec<String> = pool.iter().map(|t| t.name.clone()).collect();
    Some(PatternHit {
        kind: PatternKind::ThreadPoolExhaustion,
        severity: severity.to_string(),
        detail: format!(
            "{} pool workers busy with 0 idle getTask ({} BLOCKED)",
            names.len(),
            blocked
        ),
        thread_names: names,
    })
}

/// Frames that indicate synchronous network / JDBC / RPC blocking.
fn is_sync_io_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "SocketInputStream",
        "SocketOutputStream",
        "SocketDispatcher",
        "PlainSocketImpl",
        "NioSocketImpl",
        "SocketChannel.read",
        "SocketChannel.write",
        "ServerSocket.accept",
        "Socket.accept",
        "HttpURLConnection",
        "HttpClient",
        "okhttp3",
        "OkHttpClient",
        "org.apache.http",
        "io.grpc",
        "java.sql.",
        "javax.sql.",
        "jdbc",
        "DriverManager.getConnection",
        "Net.connect",
        "Socket.connect",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

fn stack_signature(frames: &[String], depth: usize) -> String {
    frames
        .iter()
        .take(depth)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Sync I/O / RPC hotspot: ≥3 threads share the same top frames and at least
/// one frame is a synchronous socket/HTTP/JDBC call.
fn detect_sync_io_hotspot(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(
                t.state.as_str(),
                "RUNNABLE" | "WAITING" | "TIMED_WAITING" | "BLOCKED"
            ) && t.stack.iter().any(|f| is_sync_io_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        let sig = stack_signature(&t.stack, 4);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }

    let mut best: Option<(String, Vec<&ThreadInfo>)> = None;
    for (sig, members) in groups {
        if members.len() < 3 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, m)| members.len() > m.len())
            .unwrap_or(true)
        {
            best = Some((sig, members));
        }
    }
    let (sig, members) = best?;
    let io_frame = members[0]
        .stack
        .iter()
        .find(|f| is_sync_io_frame(f))
        .cloned()
        .unwrap_or_else(|| sig.clone());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::SyncIoHotspot,
        severity: severity.to_string(),
        detail: format!(
            "{} threads share sync I/O stack near {}",
            names.len(),
            io_frame
        ),
        thread_names: names,
    })
}

/// Frames that mean the lock owner is blocked instead of doing useful CPU work.
fn is_blocking_owner_frame(frame: &str) -> bool {
    if is_sync_io_frame(frame) {
        return true;
    }
    const NEEDLES: &[&str] = &[
        "Thread.sleep",
        "Thread.sleep0",
        "Object.wait",
        "Object.wait0",
        "Unsafe.park",
        "LockSupport.park",
        "ConditionObject.await",
        "Condition.await",
        "CountDownLatch.await",
        "CyclicBarrier.await",
        "Semaphore.acquire",
        "LinkedBlockingQueue.take",
        "ArrayBlockingQueue.take",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

/// Dangerous hot-lock owner: hottest contended lock is held by a thread whose
/// stack shows a blocking call (sleep / park / sync I/O) while waiters are BLOCKED.
fn detect_dangerous_hot_lock_owner(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let mut lock_owner: BTreeMap<String, String> = BTreeMap::new();
    for t in threads {
        for lock in &t.held_locks {
            lock_owner
                .entry(lock.clone())
                .or_insert_with(|| t.name.clone());
        }
    }

    let mut waiters_by_lock: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in threads {
        if t.state != "BLOCKED" {
            continue;
        }
        if let Some(lock) = &t.waiting_on {
            waiters_by_lock
                .entry(lock.clone())
                .or_default()
                .push(t.name.clone());
        }
    }

    let mut best: Option<(String, String, Vec<String>)> = None; // lock, owner, waiters
    for (lock, waiters) in &waiters_by_lock {
        if waiters.is_empty() {
            continue;
        }
        let Some(owner) = lock_owner.get(lock) else {
            continue;
        };
        let better = best
            .as_ref()
            .map(|(_, _, w)| waiters.len() > w.len())
            .unwrap_or(true);
        if better {
            best = Some((lock.clone(), owner.clone(), waiters.clone()));
        }
    }
    let (lock, owner_name, waiters) = best?;
    let owner = threads.iter().find(|t| t.name == owner_name)?;
    let blocking = owner.stack.iter().find(|f| is_blocking_owner_frame(f))?;

    let mut names = vec![owner_name.clone()];
    names.extend(waiters.iter().cloned());
    let severity = if waiters.len() >= 2 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::DangerousHotLockOwner,
        severity: severity.to_string(),
        detail: format!(
            "lock {} held by {} while blocked in {} ({} waiter(s))",
            lock,
            owner_name,
            blocking,
            waiters.len()
        ),
        thread_names: names,
    })
}

/// Frames typical of Hikari / DBCP / Druid / mock pool borrow waits.
fn is_connection_pool_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "HikariPool",
        "HikariDataSource",
        "HikariPool.getConnection",
        "com.zaxxer.hikari",
        "borrowObject",
        "BasicDataSource.getConnection",
        "PoolingDataSource.getConnection",
        "DruidDataSource.getConnection",
        "DruidAbstractDataSource",
        "ConnectionPoolStarve",
        "HikariDataSource.getConnection",
        "HikariDataSource.borrowObject",
    ];
    if NEEDLES.iter().any(|n| frame.contains(n)) {
        return true;
    }
    // Generic getConnection on a *DataSource / *Pool type, but not DriverManager.
    (frame.contains(".getConnection(") || frame.contains(".getConnection)"))
        && (frame.contains("DataSource")
            || frame.contains("Pool")
            || frame.contains("Hikari")
            || frame.contains("Druid")
            || frame.contains("ConnectionPoolStarve"))
}

/// Connection-pool borrow blocking: ≥3 threads wait in pool getConnection/borrow stacks.
fn detect_connection_pool_borrow(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(
                t.state.as_str(),
                "WAITING" | "TIMED_WAITING" | "BLOCKED"
            ) && t.stack.iter().any(|f| is_connection_pool_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        let sig = stack_signature(&t.stack, 4);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }

    let mut best: Option<(String, Vec<&ThreadInfo>)> = None;
    for (sig, members) in groups {
        if members.len() < 3 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, m)| members.len() > m.len())
            .unwrap_or(true)
        {
            best = Some((sig, members));
        }
    }
    // Also accept ≥3 pool-borrow threads even if signatures differ slightly
    // (e.g. getConnection vs borrowObject wrapper depth).
    let (sig, members) = match best {
        Some(v) => v,
        None if candidates.len() >= 3 => (
            stack_signature(&candidates[0].stack, 3),
            candidates.clone(),
        ),
        None => return None,
    };

    let pool_frame = members[0]
        .stack
        .iter()
        .find(|f| is_connection_pool_frame(f))
        .cloned()
        .unwrap_or(sig);
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::ConnectionPoolBorrow,
        severity: severity.to_string(),
        detail: format!(
            "{} threads blocked borrowing near {}",
            names.len(),
            pool_frame
        ),
        thread_names: names,
    })
}

/// Frames for Future.get / CountDownLatch.await / CyclicBarrier.await wait trees.
fn is_future_latch_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "CompletableFuture.get",
        "CompletableFuture.waitingGet",
        "CompletableFuture.getNow",
        "FutureTask.get",
        "FutureTask.awaitDone",
        "Future.get",
        "CountDownLatch.await",
        "CyclicBarrier.await",
        "CyclicBarrier.dowait",
        "FutureLatchDeadlock",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

fn future_latch_kind_label(frame: &str) -> &'static str {
    if frame.contains("CyclicBarrier") {
        "CyclicBarrier.await"
    } else if frame.contains("CountDownLatch") {
        "CountDownLatch.await"
    } else if frame.contains("CompletableFuture")
        || frame.contains("FutureTask")
        || frame.contains("Future.get")
    {
        "Future.get"
    } else {
        "Future/Latch await"
    }
}

/// Future/Latch wait tree: ≥2 threads WAITING/TIMED_WAITING in Future.get /
/// CountDownLatch.await / CyclicBarrier.await (logical-deadlock style waits).
fn detect_future_latch_wait_tree(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(t.state.as_str(), "WAITING" | "TIMED_WAITING")
                && t.stack.iter().any(|f| is_future_latch_frame(f))
        })
        .collect();
    if candidates.len() < 2 {
        return None;
    }

    let mut kinds: BTreeSet<&'static str> = BTreeSet::new();
    for t in &candidates {
        if let Some(frame) = t.stack.iter().find(|f| is_future_latch_frame(f)) {
            kinds.insert(future_latch_kind_label(frame));
        }
    }

    let sample = candidates[0]
        .stack
        .iter()
        .find(|f| is_future_latch_frame(f))
        .cloned()
        .unwrap_or_else(|| "Future/Latch await".to_string());
    let names: Vec<String> = candidates.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 3 || kinds.len() >= 2 {
        "critical"
    } else {
        "warning"
    };
    let kind_list = kinds.into_iter().collect::<Vec<_>>().join("+");
    Some(PatternHit {
        kind: PatternKind::FutureLatchWaitTree,
        severity: severity.to_string(),
        detail: format!(
            "{} threads in Future/Latch wait tree near {} ({})",
            names.len(),
            sample,
            if kind_list.is_empty() {
                "await"
            } else {
                &kind_list
            }
        ),
        thread_names: names,
    })
}

/// Frames typical of Log4j / Logback / JUL-style synchronized appenders.
fn is_logging_appender_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "OutputStreamAppender",
        "AbstractOutputStreamAppender",
        "ConsoleAppender",
        "FileAppender",
        "RollingFileAppender",
        "WriterAppender",
        "AppenderSkeleton",
        "AppenderBase.doAppend",
        "AppenderAttachableImpl",
        "callAppenders",
        "ch.qos.logback",
        "org.apache.log4j",
        "org.apache.logging.log4j.core.appender",
        "LoggingAppenderContention",
    ];
    if NEEDLES.iter().any(|n| frame.contains(n)) {
        return true;
    }
    // Generic doAppend on an *Appender type.
    frame.contains(".doAppend(") && frame.contains("Appender")
}

/// Logging-appender contention: ≥3 threads stuck in appender/logger stacks
/// (typically one holder sleeping in append + BLOCKED waiters on the same lock).
fn detect_logging_appender_contention(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(
                t.state.as_str(),
                "BLOCKED" | "WAITING" | "TIMED_WAITING"
            ) && t.stack.iter().any(|f| is_logging_appender_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let blocked = candidates.iter().filter(|t| t.state == "BLOCKED").count();
    // Prefer a clear contention signature: at least two BLOCKED waiters.
    if blocked < 2 {
        return None;
    }

    let sample = candidates
        .iter()
        .find(|t| t.state == "BLOCKED")
        .or(candidates.first())
        .and_then(|t| t.stack.iter().find(|f| is_logging_appender_frame(f)))
        .cloned()
        .unwrap_or_else(|| "appender".to_string());
    let names: Vec<String> = candidates.iter().map(|t| t.name.clone()).collect();
    let severity = if blocked >= 3 || names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::LoggingAppenderContention,
        severity: severity.to_string(),
        detail: format!(
            "{} threads contending on logging appender near {} ({} BLOCKED)",
            names.len(),
            sample,
            blocked
        ),
        thread_names: names,
    })
}

/// Frames that mean the thread is blocked / yielding, not CPU-spinning.
fn is_yielding_or_blocking_frame(frame: &str) -> bool {
    // Reuse owner-blocking + sync-I/O needles (sleep/park/wait/socket/…).
    is_blocking_owner_frame(frame)
}

/// Busy-wait / CPU spin hotspot: ≥3 RUNNABLE threads share a tight top-stack
/// signature with no park/wait/sleep/I/O frames (feat-034).
fn detect_busy_wait_spin_hotspot(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            t.state == "RUNNABLE"
                && t.stack.len() >= 2
                && !t
                    .stack
                    .first()
                    .map(|f| f.contains("java.lang.Thread.run"))
                    .unwrap_or(true)
                && !t.stack.iter().any(|f| is_yielding_or_blocking_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        // Tight signature: top 3 frames capture the spin loop body.
        let sig = stack_signature(&t.stack, 3);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }

    let mut best: Option<(String, Vec<&ThreadInfo>)> = None;
    for (sig, members) in groups {
        if members.len() < 3 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, m)| members.len() > m.len())
            .unwrap_or(true)
        {
            best = Some((sig, members));
        }
    }
    let (sig, members) = best?;
    let top = members[0]
        .stack
        .first()
        .cloned()
        .unwrap_or_else(|| sig.clone());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::BusyWaitSpinHotspot,
        severity: severity.to_string(),
        detail: format!(
            "{} RUNNABLE threads share busy-wait/spin stack near {}",
            names.len(),
            top
        ),
        thread_names: names,
    })
}

/// Frames for `Condition.await` / AQS `ConditionObject` parking.
fn is_condition_await_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "ConditionObject.await",
        "ConditionObject.awaitNanos",
        "ConditionObject.awaitUntil",
        "Condition.await",
        "ReentrantLock$ConditionObject",
        "ConditionStarvation",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

fn is_condition_signal_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "ConditionObject.signal",
        "ConditionObject.signalAll",
        "Condition.signal",
        "Condition.signalAll",
        ".signal(",
        ".signalAll(",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

/// Condition / park starvation: ≥3 WAITING/TIMED_WAITING threads in
/// Condition.await / ConditionObject park stacks, with no RUNNABLE signaler.
fn detect_condition_park_starvation(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(t.state.as_str(), "WAITING" | "TIMED_WAITING")
                && t.stack.iter().any(|f| is_condition_await_frame(f))
                && !stack_has_idle_get_task(&t.stack)
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    // Prefer a shared top-stack signature (≥3); else accept ≥3 condition waiters.
    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        let sig = stack_signature(&t.stack, 4);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }
    let members: Vec<&ThreadInfo> = groups
        .into_iter()
        .filter(|(_, m)| m.len() >= 3)
        .max_by_key(|(_, m)| m.len())
        .map(|(_, m)| m)
        .unwrap_or_else(|| candidates.clone());
    if members.len() < 3 {
        return None;
    }

    // Starvation signal: nobody RUNNABLE is in signal/signalAll.
    let has_signaler = threads.iter().any(|t| {
        t.state == "RUNNABLE" && t.stack.iter().any(|f| is_condition_signal_frame(f))
    });
    if has_signaler {
        return None;
    }

    let sample = members[0]
        .stack
        .iter()
        .find(|f| is_condition_await_frame(f))
        .cloned()
        .unwrap_or_else(|| "Condition.await".to_string());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::ConditionParkStarvation,
        severity: severity.to_string(),
        detail: format!(
            "{} threads parked on Condition/park near {} with no RUNNABLE signaler",
            names.len(),
            sample
        ),
        thread_names: names,
    })
}

/// Build lock-acquisition order for a thread: outermost held lock first, then
/// any `waiting to lock` target as the next intended acquisition.
fn lock_acquisition_order(t: &ThreadInfo) -> Vec<String> {
    // jstack lists innermost `locked` first; reverse to get acquire order.
    let mut order: Vec<String> = t.held_locks.iter().rev().cloned().collect();
    if let Some(w) = &t.waiting_on {
        if !order.iter().any(|l| l == w) {
            order.push(w.clone());
        }
    }
    order
}

/// Nested lock-order inconsistency: observe both A→B and B→A acquisition
/// orders across threads (risk of deadlock; may or may not already cycle).
fn detect_lock_order_inconsistency(threads: &[ThreadInfo]) -> Option<PatternHit> {
    // edge (before, after) -> thread names that witnessed it
    let mut edges: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for t in threads {
        let order = lock_acquisition_order(t);
        if order.len() < 2 {
            continue;
        }
        for i in 0..order.len() {
            for j in (i + 1)..order.len() {
                let before = order[i].clone();
                let after = order[j].clone();
                if before == after {
                    continue;
                }
                edges
                    .entry((before, after))
                    .or_default()
                    .push(t.name.clone());
            }
        }
    }

    let mut best: Option<(String, String, Vec<String>)> = None;
    let keys: Vec<(String, String)> = edges.keys().cloned().collect();
    for (a, b) in &keys {
        if a >= b {
            // Only consider each unordered pair once (canonical a < b).
            continue;
        }
        let Some(fwd) = edges.get(&(a.clone(), b.clone())) else {
            continue;
        };
        let Some(rev) = edges.get(&(b.clone(), a.clone())) else {
            continue;
        };
        let mut names: BTreeSet<String> = BTreeSet::new();
        names.extend(fwd.iter().cloned());
        names.extend(rev.iter().cloned());
        let names: Vec<String> = names.into_iter().collect();
        if names.len() < 2 {
            continue;
        }
        let better = best
            .as_ref()
            .map(|(_, _, n)| names.len() > n.len())
            .unwrap_or(true);
        if better {
            best = Some((a.clone(), b.clone(), names));
        }
    }
    let (a, b, names) = best?;
    let severity = if names.len() >= 3 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::LockOrderInconsistency,
        severity: severity.to_string(),
        detail: format!(
            "inconsistent lock order {}↔{} across {} threads (deadlock risk)",
            a,
            b,
            names.len()
        ),
        thread_names: names,
    })
}

fn is_ref_mgmt_thread(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "finalizer"
        || n == "reference handler"
        || n == "common-cleaner"
        || n.starts_with("cleaner-")
}

fn is_idle_ref_mgmt_stack(stack: &[String]) -> bool {
    let idle = stack.iter().any(|f| {
        f.contains("ReferenceQueue.remove")
            || f.contains("ReferenceHandler.run")
            || f.contains("CleanerImpl.run")
            || f.contains("Common-Cleaner")
    });
    let working = stack.iter().any(|f| {
        f.contains(".finalize(")
            || f.contains("Finalizer.runFinalizer")
            || f.contains("FinalizerPressure")
            || f.contains("Cleaner.clean")
    });
    idle && !working
}

fn is_finalize_work_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        ".finalize(",
        "Finalizer.runFinalizer",
        "Finalizer$FinalizerThread",
        "FinalizerPressure",
        "Cleaner.clean",
        "CleanerImpl",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

/// Finalizer / Reference Handler pressure: system ref-management thread is
/// busy or BLOCKED (not idle on ReferenceQueue.remove), often while holding or
/// waiting on an application lock (feat-037).
fn detect_finalizer_pressure(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let ref_threads: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| is_ref_mgmt_thread(&t.name))
        .collect();
    if ref_threads.is_empty() {
        return None;
    }

    let busy: Vec<&ThreadInfo> = ref_threads
        .iter()
        .copied()
        .filter(|t| {
            if is_idle_ref_mgmt_stack(&t.stack) {
                return false;
            }
            match t.state.as_str() {
                "BLOCKED" => true,
                "RUNNABLE" | "TIMED_WAITING" | "WAITING" => {
                    t.stack.iter().any(|f| is_finalize_work_frame(f))
                        || (!t.held_locks.is_empty() && t.state != "WAITING")
                        || t.state == "BLOCKED"
                }
                _ => false,
            }
        })
        .collect();
    if busy.is_empty() {
        return None;
    }

    let mut lock_owner: BTreeMap<String, String> = BTreeMap::new();
    for t in threads {
        for lock in &t.held_locks {
            lock_owner
                .entry(lock.clone())
                .or_insert_with(|| t.name.clone());
        }
    }

    // Application impact: Finalizer BLOCKED, or app threads blocked on a lock
    // it holds, or Finalizer blocked on a lock held by a non-ref thread.
    let mut impact_names: BTreeSet<String> = BTreeSet::new();
    for t in &busy {
        impact_names.insert(t.name.clone());
        if t.state == "BLOCKED" {
            if let Some(lock) = &t.waiting_on {
                if let Some(owner) = lock_owner.get(lock) {
                    if !is_ref_mgmt_thread(owner) {
                        impact_names.insert(owner.clone());
                    }
                }
            }
        }
        for lock in &t.held_locks {
            for w in threads {
                if w.waiting_on.as_ref() == Some(lock) && !is_ref_mgmt_thread(&w.name) {
                    impact_names.insert(w.name.clone());
                }
            }
        }
    }

    let blocked_busy = busy.iter().filter(|t| t.state == "BLOCKED").count();
    let has_finalize = busy
        .iter()
        .any(|t| t.stack.iter().any(|f| is_finalize_work_frame(f)));
    // Require a clear pressure signal: blocked ref thread and/or app impact
    // and/or explicit finalize work (not merely a non-idle name match).
    if blocked_busy == 0 && impact_names.len() <= busy.len() && !has_finalize {
        return None;
    }
    // If only busy names with finalize work but no BLOCKED and no extra app
    // waiters, still report (Finalizer stuck in finalize/sleep).
    let app_impact = impact_names.len() > busy.len() || blocked_busy > 0;
    if !has_finalize && !app_impact {
        return None;
    }

    let sample = busy[0]
        .stack
        .iter()
        .find(|f| is_finalize_work_frame(f))
        .cloned()
        .unwrap_or_else(|| busy[0].name.clone());
    let mut names: Vec<String> = impact_names.into_iter().collect();
    names.sort();
    let severity = if blocked_busy > 0 || names.len() >= 3 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::FinalizerPressure,
        severity: severity.to_string(),
        detail: format!(
            "Finalizer/Reference Handler pressure near {} ({} ref-mgmt busy, {} BLOCKED)",
            sample,
            busy.len(),
            blocked_busy
        ),
        thread_names: names,
    })
}

/// Mirror of `web/src/analysisUi.ts` `isJvmNoise` — skip GC/compiler/Finalizer/…
fn is_jvm_noise_thread(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    const EXACT: &[&str] = &[
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
    if EXACT.iter().any(|e| n == *e) {
        return true;
    }
    n.starts_with("c1 compiler")
        || n.starts_with("c2 compiler")
        || n.starts_with("gc ")
        || n.contains("gc thread")
        || n.starts_with("g1 ")
        || n.starts_with("gang worker")
        || n.contains("parallelgc")
        || n.starts_with("jvmci")
        || n.contains("cleaner-")
}

fn is_sleep_frame(frame: &str) -> bool {
    frame.contains("Thread.sleep") || frame.contains("Thread.sleep0")
}

/// Thread.sleep-as-scheduler: ≥3 non-JVM-noise TIMED_WAITING business threads
/// share a stack dominated by Thread.sleep (feat-038).
fn detect_sleep_as_scheduler(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            !is_jvm_noise_thread(&t.name)
                && t.state == "TIMED_WAITING"
                && t.stack.iter().any(|f| is_sleep_frame(f))
                // Avoid Condition TIMED_WAITING park stacks (feat-035).
                && !t.stack.iter().any(|f| is_condition_await_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        let sig = stack_signature(&t.stack, 3);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }

    let mut best: Option<(String, Vec<&ThreadInfo>)> = None;
    for (sig, members) in groups {
        if members.len() < 3 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, m)| members.len() > m.len())
            .unwrap_or(true)
        {
            best = Some((sig, members));
        }
    }
    let (sig, members) = best?;
    let sample = members[0]
        .stack
        .iter()
        .find(|f| is_sleep_frame(f))
        .cloned()
        .unwrap_or_else(|| sig.clone());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::SleepAsScheduler,
        severity: severity.to_string(),
        detail: format!(
            "{} TIMED_WAITING business threads use Thread.sleep as a scheduler near {}",
            names.len(),
            sample
        ),
        thread_names: names,
    })
}

/// Tomcat `http-nio-*-exec-*`, Jetty `qtp*`, Netty `*nioEventLoop*` worker names.
fn framework_worker_family(name: &str) -> Option<&'static str> {
    let tomcat = Regex::new(r"(?i)^http-nio-\d+-exec-\d+$").unwrap();
    if tomcat.is_match(name) {
        return Some("tomcat");
    }
    // Broader Tomcat connector exec naming.
    if name.contains("http-nio-") && name.contains("-exec-") {
        return Some("tomcat");
    }
    let jetty = Regex::new(r"^qtp\d+-\d+$").unwrap();
    if jetty.is_match(name) || (name.starts_with("qtp") && name.contains('-')) {
        return Some("jetty");
    }
    let lower = name.to_ascii_lowercase();
    if lower.contains("nioeventloop") {
        return Some("netty");
    }
    None
}

/// Healthy idle stacks for framework workers (waiting for the next request/event).
fn is_framework_idle_stack(stack: &[String]) -> bool {
    stack.iter().any(|f| {
        f.contains("ThreadPoolExecutor.getTask")
            || f.contains("TaskQueue.take")
            || f.contains("QueuedThreadPool.idleJob")
            || f.contains("NioEventLoop.select")
            || f.contains("Selector.select")
            || f.contains("SelectorImpl.select")
            || f.contains("epollWait")
            || f.contains("KQueue.kevent")
    })
}

/// Framework worker-pool saturation: ≥3 Tomcat/Jetty/Netty workers share the
/// same non-idle blocking work stack (feat-039).
fn detect_framework_pool_saturation(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let mut by_family: BTreeMap<&'static str, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in threads {
        if let Some(fam) = framework_worker_family(&t.name) {
            if !is_framework_idle_stack(&t.stack) {
                by_family.entry(fam).or_default().push(t);
            }
        }
    }

    let mut best: Option<(&'static str, String, Vec<&ThreadInfo>)> = None;
    for (fam, members) in by_family {
        if members.len() < 3 {
            continue;
        }
        let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
        for t in &members {
            let sig = stack_signature(&t.stack, 4);
            if sig.is_empty() {
                continue;
            }
            groups.entry(sig).or_default().push(*t);
        }
        for (sig, cluster) in groups {
            if cluster.len() < 3 {
                continue;
            }
            let better = best
                .as_ref()
                .map(|(_, _, m)| cluster.len() > m.len())
                .unwrap_or(true);
            if better {
                best = Some((fam, sig, cluster));
            }
        }
    }
    let (fam, sig, members) = best?;
    let blocked = members.iter().filter(|t| t.state == "BLOCKED").count();
    let sample = members[0]
        .stack
        .first()
        .cloned()
        .unwrap_or_else(|| sig.clone());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if blocked >= 2 || names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::FrameworkPoolSaturation,
        severity: severity.to_string(),
        detail: format!(
            "{} {} framework workers saturated on shared work near {} ({} BLOCKED)",
            names.len(),
            fam,
            sample,
            blocked
        ),
        thread_names: names,
    })
}

/// Frames that indicate DNS / name-resolution work (InetAddress or JNDI DNS).
fn is_dns_resolution_frame(frame: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "InetAddress.getByName",
        "InetAddress.getAllByName",
        "InetAddress$NameServiceAddresses",
        "InetAddress$PlatformNameService",
        "InetAddressResolver",
        "lookupAllHostAddr",
        "Inet4AddressImpl",
        "Inet6AddressImpl",
        "com.sun.jndi.dns",
        "DnsClient",
        "DnsContext",
        "DnsResolutionStall",
    ];
    NEEDLES.iter().any(|n| frame.contains(n))
}

/// DNS / name-resolution stall: ≥3 threads share stacks stuck in InetAddress
/// or DNS Resolver frames (feat-040).
fn detect_dns_resolution_stall(threads: &[ThreadInfo]) -> Option<PatternHit> {
    let candidates: Vec<&ThreadInfo> = threads
        .iter()
        .filter(|t| {
            matches!(
                t.state.as_str(),
                "RUNNABLE" | "WAITING" | "TIMED_WAITING" | "BLOCKED"
            ) && t.stack.iter().any(|f| is_dns_resolution_frame(f))
        })
        .collect();
    if candidates.len() < 3 {
        return None;
    }

    let mut groups: BTreeMap<String, Vec<&ThreadInfo>> = BTreeMap::new();
    for t in &candidates {
        let sig = stack_signature(&t.stack, 4);
        if sig.is_empty() {
            continue;
        }
        groups.entry(sig).or_default().push(*t);
    }

    let mut best: Option<(String, Vec<&ThreadInfo>)> = None;
    for (sig, members) in groups {
        if members.len() < 3 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, m)| members.len() > m.len())
            .unwrap_or(true)
        {
            best = Some((sig, members));
        }
    }
    let (sig, members) = best?;
    let sample = members[0]
        .stack
        .iter()
        .find(|f| is_dns_resolution_frame(f))
        .cloned()
        .unwrap_or_else(|| sig.clone());
    let names: Vec<String> = members.iter().map(|t| t.name.clone()).collect();
    let severity = if names.len() >= 5 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::DnsResolutionStall,
        severity: severity.to_string(),
        detail: format!(
            "{} threads stalled in DNS/name-resolution near {}",
            names.len(),
            sample
        ),
        thread_names: names,
    })
}

fn app_thread_count(a: &Analysis) -> usize {
    a.threads
        .iter()
        .filter(|t| !is_jvm_noise_thread(&t.name))
        .count()
}

/// Thread leak: non-JVM-noise thread count grows across an ordered dump series.
fn detect_thread_leak(dumps: &[Analysis]) -> Option<PatternHit> {
    if dumps.len() < 2 {
        return None;
    }
    let counts: Vec<usize> = dumps.iter().map(app_thread_count).collect();
    for w in counts.windows(2) {
        if w[1] < w[0] {
            return None;
        }
    }
    let first = *counts.first()?;
    let last = *counts.last()?;
    let growth = last.saturating_sub(first);
    if growth < 3 {
        return None;
    }

    let first_names: BTreeSet<&str> = dumps[0].threads.iter().map(|t| t.name.as_str()).collect();
    let mut new_names: Vec<String> = dumps
        .last()?
        .threads
        .iter()
        .filter(|t| !is_jvm_noise_thread(&t.name) && !first_names.contains(t.name.as_str()))
        .map(|t| t.name.clone())
        .collect();
    new_names.sort();
    new_names.dedup();
    if new_names.is_empty() {
        new_names = dumps
            .last()?
            .threads
            .iter()
            .filter(|t| !is_jvm_noise_thread(&t.name))
            .map(|t| t.name.clone())
            .take(8)
            .collect();
    } else {
        new_names.truncate(8);
    }

    let count_path = counts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" → ");
    let severity = if growth >= 10 { "critical" } else { "warning" };
    Some(PatternHit {
        kind: PatternKind::ThreadLeak,
        severity: severity.to_string(),
        detail: format!(
            "app thread count {} across {} dumps (+{})",
            count_path,
            dumps.len(),
            growth
        ),
        thread_names: new_names,
    })
}

/// Livelock: ≥2 non-noise threads present in every dump keep changing stacks.
fn detect_livelock(dumps: &[Analysis]) -> Option<PatternHit> {
    if dumps.len() < 2 {
        return None;
    }

    let mut by_name: BTreeMap<String, Vec<Option<String>>> = BTreeMap::new();
    for (i, dump) in dumps.iter().enumerate() {
        for t in &dump.threads {
            if is_jvm_noise_thread(&t.name) || t.state == "TERMINATED" {
                continue;
            }
            let entry = by_name
                .entry(t.name.clone())
                .or_insert_with(|| vec![None; dumps.len()]);
            if !t.stack.is_empty() {
                entry[i] = Some(stack_signature(&t.stack, 4));
            }
        }
    }

    let mut oscillating: Vec<String> = Vec::new();
    for (name, sigs) in &by_name {
        if sigs.iter().any(|s| s.is_none()) {
            continue;
        }
        let present: Vec<&String> = sigs.iter().filter_map(|s| s.as_ref()).collect();
        if present.len() < dumps.len() {
            continue;
        }
        let mut changed = false;
        for w in present.windows(2) {
            if w[0] != w[1] {
                changed = true;
                break;
            }
        }
        if !changed {
            continue;
        }
        let mut uniq: BTreeSet<&str> = BTreeSet::new();
        for s in &present {
            uniq.insert(s.as_str());
        }
        if uniq.len() < 2 {
            continue;
        }
        oscillating.push(name.clone());
    }

    if oscillating.len() < 2 {
        return None;
    }
    oscillating.sort();
    let severity = if oscillating.len() >= 4 {
        "critical"
    } else {
        "warning"
    };
    Some(PatternHit {
        kind: PatternKind::Livelock,
        severity: severity.to_string(),
        detail: format!(
            "{} threads changing stacks across {} dumps without settling (livelock)",
            oscillating.len(),
            dumps.len()
        ),
        thread_names: oscillating,
    })
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

    const MXBEAN_CONTENTION: &str = r#""waiter-1" Id=11 BLOCKED on java.lang.Object@53d8d10a owned by "holder" Id=9
	at MxBeanDump.lambda$main$1(MxBeanDump.java:22)
	-  blocked on java.lang.Object@53d8d10a
	at java.lang.Thread.run(Thread.java:750)

"waiter-0" Id=10 BLOCKED on java.lang.Object@53d8d10a owned by "holder" Id=9
	at MxBeanDump.lambda$main$1(MxBeanDump.java:22)
	-  blocked on java.lang.Object@53d8d10a
	at java.lang.Thread.run(Thread.java:750)

"holder" Id=9 TIMED_WAITING
	at java.lang.Thread.sleep(Native Method)
	-  locked java.lang.Object@53d8d10a
	at java.lang.Thread.run(Thread.java:750)
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
        // Condition `waiting on` must NOT become a contention edge.
        assert!(a.blocked_edges.is_empty());
    }

    #[test]
    fn detects_mxbean_format_lock_contentions() {
        let a = analyze(MXBEAN_CONTENTION);
        assert_eq!(a.format, DumpFormat::ThreadMxBean);
        let holder = a.threads.iter().find(|t| t.name == "holder").unwrap();
        assert_eq!(
            holder.held_locks,
            vec!["java.lang.Object@53d8d10a".to_string()]
        );
        let waiters: Vec<_> = a
            .threads
            .iter()
            .filter(|t| t.name.starts_with("waiter-"))
            .collect();
        assert_eq!(waiters.len(), 2);
        for w in &waiters {
            assert_eq!(w.state, "BLOCKED");
            assert_eq!(
                w.waiting_on.as_deref(),
                Some("java.lang.Object@53d8d10a")
            );
        }
        assert_eq!(a.blocked_edges.len(), 2);
        for edge in &a.blocked_edges {
            assert!(edge.blocked_thread.starts_with("waiter-"));
            assert_eq!(edge.lock, "java.lang.Object@53d8d10a");
            assert_eq!(edge.owner_thread.as_deref(), Some("holder"));
        }
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
        assert_eq!(main.stack, vec!["com.example.App.run(App.java:10)".to_string()]);
        assert_eq!(
            main.waiting_on.as_deref(),
            Some("0x000000076ab00000")
        );
    }

    #[test]
    fn captures_top_stack_frames() {
        let a = analyze(JSTACK_SAMPLE);
        let worker = a.threads.iter().find(|t| t.name == "worker").unwrap();
        assert_eq!(worker.stack_depth, 1);
        assert!(!worker.stack.is_empty());
        assert!(worker.stack[0].contains("com.example.Worker.work"));
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

    // feat-008: real dumps captured with Temurin 8/11/17/21 via jenv
    // (see tests/fixtures/java-versions/FORMAT_DIFFS.md).
    const JV_JSTACK_CONTENTION: [&str; 4] = [
        include_str!("../tests/fixtures/java-versions/java8-jstack-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java11-jstack-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java17-jstack-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java21-jstack-contention.txt"),
    ];
    const JV_JSTACK_DEADLOCK: [&str; 4] = [
        include_str!("../tests/fixtures/java-versions/java8-jstack-deadlock.txt"),
        include_str!("../tests/fixtures/java-versions/java11-jstack-deadlock.txt"),
        include_str!("../tests/fixtures/java-versions/java17-jstack-deadlock.txt"),
        include_str!("../tests/fixtures/java-versions/java21-jstack-deadlock.txt"),
    ];
    const JV_MXBEAN_CONTENTION: [&str; 4] = [
        include_str!("../tests/fixtures/java-versions/java8-mxbean-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java11-mxbean-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java17-mxbean-contention.txt"),
        include_str!("../tests/fixtures/java-versions/java21-mxbean-contention.txt"),
    ];
    const JV_LABELS: [&str; 4] = ["8", "11", "17", "21"];

    #[test]
    fn detects_java_version_support() {
        for (i, label) in JV_LABELS.iter().enumerate() {
            // --- jstack lock contention ---
            let a = analyze(JV_JSTACK_CONTENTION[i]);
            assert_eq!(a.format, DumpFormat::Jstack, "java{label} jstack format");
            let holder = a
                .threads
                .iter()
                .find(|t| t.name == "holder")
                .unwrap_or_else(|| panic!("java{label}: missing holder"));
            assert_eq!(holder.state, "TIMED_WAITING");
            assert_eq!(holder.held_locks.len(), 1, "java{label}: holder lock");
            assert!(holder.id.is_some(), "java{label}: jstack #N id");

            let waiters: Vec<_> = a
                .threads
                .iter()
                .filter(|t| t.name.starts_with("waiter-"))
                .collect();
            assert_eq!(waiters.len(), 2, "java{label}: waiter count");
            for w in &waiters {
                assert_eq!(w.state, "BLOCKED");
                assert_eq!(w.waiting_on.as_ref(), Some(&holder.held_locks[0]));
            }
            assert!(
                a.blocked_edges.len() >= 2,
                "java{label}: expected contention edges, got {}",
                a.blocked_edges.len()
            );
            for edge in a
                .blocked_edges
                .iter()
                .filter(|e| e.blocked_thread.starts_with("waiter-"))
            {
                assert_eq!(edge.owner_thread.as_deref(), Some("holder"));
            }

            // --- jstack deadlock ---
            let d = analyze(JV_JSTACK_DEADLOCK[i]);
            assert_eq!(d.format, DumpFormat::Jstack, "java{label} deadlock format");
            let phantom = d
                .threads
                .iter()
                .filter(|t| t.name.starts_with("deadlock-") && t.state == "UNKNOWN")
                .count();
            assert_eq!(phantom, 0, "java{label}: deadlock summary leaked as threads");
            assert_eq!(d.deadlocks.len(), 1, "java{label}: deadlock cycle count");
            assert_eq!(d.deadlocks[0].threads.len(), 3, "java{label}: cycle size");
            let members: BTreeSet<&str> =
                d.deadlocks[0].threads.iter().map(|s| s.as_str()).collect();
            for name in ["deadlock-0", "deadlock-1", "deadlock-2"] {
                assert!(
                    members.contains(name),
                    "java{label}: missing {name} in cycle"
                );
            }

            // --- ThreadMXBean: format + thread split + lock contention ---
            let m = analyze(JV_MXBEAN_CONTENTION[i]);
            assert_eq!(
                m.format,
                DumpFormat::ThreadMxBean,
                "java{label} mxbean format"
            );
            let holder = m
                .threads
                .iter()
                .find(|t| t.name == "holder")
                .unwrap_or_else(|| panic!("java{label} mxbean: missing holder"));
            assert_eq!(holder.state, "TIMED_WAITING");
            assert!(holder.id.is_some(), "java{label} mxbean: Id=N");
            assert_eq!(
                holder.held_locks.len(),
                1,
                "java{label} mxbean: holder lock"
            );
            let waiters: Vec<_> = m
                .threads
                .iter()
                .filter(|t| t.name.starts_with("waiter-"))
                .collect();
            assert_eq!(waiters.len(), 2, "java{label} mxbean: waiter count");
            for w in &waiters {
                assert_eq!(w.state, "BLOCKED", "java{label} mxbean: waiter state");
                assert_eq!(
                    w.waiting_on.as_ref(),
                    Some(&holder.held_locks[0]),
                    "java{label} mxbean: waiter lock"
                );
            }
            let edges: Vec<_> = m
                .blocked_edges
                .iter()
                .filter(|e| e.blocked_thread.starts_with("waiter-"))
                .collect();
            assert_eq!(edges.len(), 2, "java{label} mxbean: contention edges");
            for edge in edges {
                assert_eq!(edge.owner_thread.as_deref(), Some("holder"));
            }
        }
    }

    // feat-009: excerpt from a real Flink/Kafka ThreadMXBean dump (tdump_15c7).
    const REAL_MXBEAN: &str = include_str!("../tests/fixtures/mxbean_real_contention.txt");

    #[test]
    fn detects_mxbean_format_lock_contentions_real_world() {
        let a = analyze(REAL_MXBEAN);
        assert_eq!(a.format, DumpFormat::ThreadMxBean);

        let kafka = a
            .blocked_edges
            .iter()
            .find(|e| e.blocked_thread.starts_with("kafka-producer-network-thread"))
            .expect("kafka contention edge");
        assert_eq!(kafka.lock, "java.lang.Object@7ec4e9a");
        assert_eq!(kafka.owner_thread.as_deref(), Some("Writer: writer (1/1)#0"));

        let log_lock =
            "org.apache.logging.log4j.core.appender.rolling.RollingFileManager@30dbe1cc";
        let log_edges: Vec<_> = a
            .blocked_edges
            .iter()
            .filter(|e| e.lock == log_lock)
            .collect();
        assert!(
            log_edges.len() >= 2,
            "expected multiple RollingFileManager waiters, got {}",
            log_edges.len()
        );
        for edge in &log_edges {
            assert_eq!(
                edge.owner_thread.as_deref(),
                Some("stage -> Timestamps/Watermarks (58/60)#0")
            );
        }

        let holder = a
            .threads
            .iter()
            .find(|t| t.name == "stage -> Timestamps/Watermarks (58/60)#0")
            .unwrap();
        assert!(holder.held_locks.iter().any(|l| l == log_lock));
    }

    // feat-025: richer "Load sample" dump shipped with the web UI.
    const WEB_SAMPLE: &str = include_str!("../web/src/sample.tdump");

    #[test]
    fn parses_web_sample_dump() {
        let a = analyze(WEB_SAMPLE);
        assert_eq!(a.format, DumpFormat::Jstack);
        assert_eq!(a.total_threads, 17);

        // 3-thread checkout deadlock cycle.
        assert_eq!(a.deadlocks.len(), 1);
        let dl = &a.deadlocks[0];
        assert_eq!(dl.threads.len(), 3);
        let members: BTreeSet<&str> = dl.threads.iter().map(|s| s.as_str()).collect();
        assert!(members.contains("order-checkout-0"));
        assert!(members.contains("order-checkout-1"));
        assert!(members.contains("order-checkout-2"));

        // Hot cache lock with 4 http-worker waiters.
        let cache_lock = "0x000000076ab20000";
        let cache_waiters: Vec<_> = a
            .blocked_edges
            .iter()
            .filter(|e| e.lock == cache_lock)
            .collect();
        assert_eq!(cache_waiters.len(), 4);
        for edge in &cache_waiters {
            assert_eq!(edge.owner_thread.as_deref(), Some("cache-writer"));
            assert!(edge.blocked_thread.starts_with("http-worker-"));
        }

        // Stack frames captured for UI expand / clusters.
        let worker = a
            .threads
            .iter()
            .find(|t| t.name == "http-worker-1")
            .expect("http-worker-1");
        assert!(!worker.stack.is_empty());
        assert_eq!(worker.waiting_on.as_deref(), Some(cache_lock));

        // Mixed states present.
        let states: BTreeSet<&str> = a.state_counts.iter().map(|s| s.state.as_str()).collect();
        for expected in ["BLOCKED", "RUNNABLE", "WAITING", "TIMED_WAITING"] {
            assert!(states.contains(expected), "missing state {expected}");
        }
    }

    // feat-028: synthetic jstack shaped like Executors.newFixedThreadPool exhaustion.
    const POOL_EXHAUSTION_SAMPLE: &str = r#"2026-07-24 12:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"pool-1-thread-1" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076ab20000> (a java.lang.Object)
        at ThreadPoolExhaustion.lambda$main$0(ThreadPoolExhaustion.java:20)
        at java.util.concurrent.Executors$RunnableAdapter.call(Executors.java:539)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1144)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)

"pool-1-thread-2" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at ThreadPoolExhaustion.lambda$main$0(ThreadPoolExhaustion.java:18)
        - waiting to lock <0x000000076ab20000> (a java.lang.Object)
        at java.util.concurrent.Executors$RunnableAdapter.call(Executors.java:539)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1144)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)

"pool-1-thread-3" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at ThreadPoolExhaustion.lambda$main$0(ThreadPoolExhaustion.java:18)
        - waiting to lock <0x000000076ab20000> (a java.lang.Object)
        at java.util.concurrent.Executors$RunnableAdapter.call(Executors.java:539)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1144)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)

"pool-1-thread-4" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting for monitor entry [0x4]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at ThreadPoolExhaustion.lambda$main$0(ThreadPoolExhaustion.java:18)
        - waiting to lock <0x000000076ab20000> (a java.lang.Object)
        at java.util.concurrent.Executors$RunnableAdapter.call(Executors.java:539)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1144)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_thread_pool_exhaustion_pattern() {
        let a = analyze(POOL_EXHAUSTION_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ThreadPoolExhaustion)
            .expect("thread-pool-exhaustion");
        assert_eq!(hit.severity, "critical");
        assert_eq!(hit.thread_names.len(), 4);
        // Same dump also shows a sleeping owner holding the contended lock.
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::DangerousHotLockOwner)
        );
    }

    #[test]
    fn detects_thread_pool_exhaustion_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/thread_pool_exhaustion_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ThreadPoolExhaustion),
            "live fixture should detect pool exhaustion"
        );
    }

    #[test]
    fn detects_sync_io_hotspot_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/sync_io_hotspot_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::SyncIoHotspot),
            "live fixture should detect sync-io hotspot"
        );
    }

    #[test]
    fn detects_dangerous_hot_lock_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/dangerous_hot_lock_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::DangerousHotLockOwner),
            "live fixture should detect dangerous hot-lock owner"
        );
    }

    #[test]
    fn detects_connection_pool_borrow_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/connection_pool_starve_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ConnectionPoolBorrow),
            "live fixture should detect connection-pool borrow blocking"
        );
    }

    #[test]
    fn detects_future_latch_wait_tree_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/future_latch_deadlock_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FutureLatchWaitTree),
            "live fixture should detect Future/Latch wait tree"
        );
    }

    #[test]
    fn detects_logging_appender_contention_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/logging_appender_contention_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::LoggingAppenderContention),
            "live fixture should detect logging-appender contention"
        );
    }

    #[test]
    fn detects_busy_wait_spin_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/busy_wait_spin_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::BusyWaitSpinHotspot),
            "live fixture should detect busy-wait/spin hotspot"
        );
    }

    #[test]
    fn detects_condition_park_starvation_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/condition_starvation_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::ConditionParkStarvation),
            "live fixture should detect Condition/park starvation"
        );
    }

    #[test]
    fn detects_lock_order_inconsistency_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/lock_order_risk_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::LockOrderInconsistency),
            "live fixture should detect lock-order inconsistency"
        );
    }

    #[test]
    fn detects_finalizer_pressure_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/finalizer_pressure_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FinalizerPressure),
            "live fixture should detect Finalizer/Reference Handler pressure"
        );
    }

    #[test]
    fn detects_sleep_as_scheduler_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/sleep_as_scheduler_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::SleepAsScheduler),
            "live fixture should detect Thread.sleep-as-scheduler"
        );
    }

    #[test]
    fn detects_framework_pool_saturation_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/framework_pool_saturation_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::FrameworkPoolSaturation),
            "live fixture should detect framework worker-pool saturation"
        );
    }

    #[test]
    fn detects_dns_resolution_stall_from_live_fixture() {
        const LIVE: &str =
            include_str!("../tests/fixtures/patterns/dns_resolution_stall_jstack.txt");
        let a = analyze(LIVE);
        assert!(
            a.patterns
                .iter()
                .any(|p| p.kind == PatternKind::DnsResolutionStall),
            "live fixture should detect DNS/name-resolution stall"
        );
    }

    #[test]
    fn idle_pool_is_not_exhaustion() {
        const IDLE: &str = r#""pool-1-thread-1" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1131)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)

"pool-1-thread-2" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1131)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)

"pool-1-thread-3" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)
        at java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1131)
        at java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:642)
        at java.lang.Thread.run(Thread.java:1583)
"#;
        let a = analyze(IDLE);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::ThreadPoolExhaustion)
        );
    }

    const SYNC_IO_SAMPLE: &str = r#"2026-07-24 13:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"rpc-client-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:318)
        at sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:346)
        at sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:796)
        at java.net.Socket$SocketInputStream.read(Socket.java:1099)
        at SyncIoHotspot.lambda$main$1(SyncIoHotspot.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"rpc-client-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:318)
        at sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:346)
        at sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:796)
        at java.net.Socket$SocketInputStream.read(Socket.java:1099)
        at SyncIoHotspot.lambda$main$1(SyncIoHotspot.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"rpc-client-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 runnable [0x3]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:318)
        at sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:346)
        at sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:796)
        at java.net.Socket$SocketInputStream.read(Socket.java:1099)
        at SyncIoHotspot.lambda$main$1(SyncIoHotspot.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"rpc-client-3" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 runnable [0x4]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:318)
        at sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:346)
        at sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:796)
        at java.net.Socket$SocketInputStream.read(Socket.java:1099)
        at SyncIoHotspot.lambda$main$1(SyncIoHotspot.java:40)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_sync_io_hotspot_pattern() {
        let a = analyze(SYNC_IO_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::SyncIoHotspot)
            .expect("sync-io-hotspot");
        assert_eq!(hit.thread_names.len(), 4);
        assert!(hit.detail.contains("Socket") || hit.detail.contains("NioSocket"));
    }

    const DANGEROUS_HOT_LOCK_SAMPLE: &str = r#"2026-07-24 13:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"lock-owner" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076ab90000> (a java.lang.Object)
        at DangerousHotLock.lambda$main$0(DangerousHotLock.java:18)
        at java.lang.Thread.run(Thread.java:1583)

"waiter-0" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at DangerousHotLock.lambda$main$1(DangerousHotLock.java:28)
        - waiting to lock <0x000000076ab90000> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)

"waiter-1" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at DangerousHotLock.lambda$main$1(DangerousHotLock.java:28)
        - waiting to lock <0x000000076ab90000> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)

"waiter-2" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting for monitor entry [0x4]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at DangerousHotLock.lambda$main$1(DangerousHotLock.java:28)
        - waiting to lock <0x000000076ab90000> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_dangerous_hot_lock_owner_pattern() {
        let a = analyze(DANGEROUS_HOT_LOCK_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::DangerousHotLockOwner)
            .expect("dangerous-hot-lock-owner");
        assert_eq!(hit.severity, "critical");
        assert!(hit.thread_names.iter().any(|n| n == "lock-owner"));
        assert_eq!(
            hit.thread_names.iter().filter(|n| n.starts_with("waiter-")).count(),
            3
        );
        assert!(hit.detail.contains("sleep"));
    }

    #[test]
    fn runnable_owner_is_not_dangerous_hot_lock() {
        // Owner is RUNNABLE doing real work while holding the lock — not "blocked owner".
        const SAFE: &str = r#""worker" #2 prio=5 os_prio=0 tid=0x2 nid=0x2 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at com.example.Worker.work(Worker.java:20)
        - locked <0x000000076ab00000> (a java.lang.Object)

"main" #1 prio=5 os_prio=0 tid=0x1 nid=0x1 waiting for monitor entry [0x1]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.run(App.java:10)
        - waiting to lock <0x000000076ab00000> (a java.lang.Object)
"#;
        let a = analyze(SAFE);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::DangerousHotLockOwner)
        );
    }

    const CONN_POOL_SAMPLE: &str = r#"2026-07-24 13:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"db-borrower-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 in Object.wait() [0x1]
   java.lang.Thread.State: WAITING (on object monitor)
        at java.lang.Object.wait(Native Method)
        - waiting on <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at java.lang.Object.wait(Object.java:338)
        at ConnectionPoolStarve$HikariDataSource.borrowObject(ConnectionPoolStarve.java:18)
        - locked <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at ConnectionPoolStarve$HikariDataSource.getConnection(ConnectionPoolStarve.java:12)
        at ConnectionPoolStarve.lambda$main$1(ConnectionPoolStarve.java:48)
        at java.lang.Thread.run(Thread.java:1583)

"db-borrower-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 in Object.wait() [0x2]
   java.lang.Thread.State: WAITING (on object monitor)
        at java.lang.Object.wait(Native Method)
        - waiting on <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at java.lang.Object.wait(Object.java:338)
        at ConnectionPoolStarve$HikariDataSource.borrowObject(ConnectionPoolStarve.java:18)
        - locked <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at ConnectionPoolStarve$HikariDataSource.getConnection(ConnectionPoolStarve.java:12)
        at ConnectionPoolStarve.lambda$main$1(ConnectionPoolStarve.java:48)
        at java.lang.Thread.run(Thread.java:1583)

"db-borrower-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 in Object.wait() [0x3]
   java.lang.Thread.State: WAITING (on object monitor)
        at java.lang.Object.wait(Native Method)
        - waiting on <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at java.lang.Object.wait(Object.java:338)
        at ConnectionPoolStarve$HikariDataSource.borrowObject(ConnectionPoolStarve.java:18)
        - locked <0x000000076abc0000> (a ConnectionPoolStarve$HikariDataSource)
        at ConnectionPoolStarve$HikariDataSource.getConnection(ConnectionPoolStarve.java:12)
        at ConnectionPoolStarve.lambda$main$1(ConnectionPoolStarve.java:48)
        at java.lang.Thread.run(Thread.java:1583)

"pool-holder" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting on condition [0x4]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at ConnectionPoolStarve.lambda$main$0(ConnectionPoolStarve.java:36)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_connection_pool_borrow_pattern() {
        let a = analyze(CONN_POOL_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ConnectionPoolBorrow)
            .expect("connection-pool-borrow");
        assert_eq!(hit.thread_names.len(), 3);
        assert!(hit.detail.contains("borrow") || hit.detail.contains("getConnection") || hit.detail.contains("Hikari"));
    }

    const FUTURE_LATCH_SAMPLE: &str = r#"2026-07-24 14:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"future-waiter-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076aaa0001> (a java.util.concurrent.CompletableFuture$Signaller)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1864)
        at java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:3780)
        at java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1898)
        at java.util.concurrent.CompletableFuture.get(CompletableFuture.java:2072)
        at FutureLatchDeadlock.lambda$main$0(FutureLatchDeadlock.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"future-waiter-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076aaa0002> (a java.util.concurrent.CompletableFuture$Signaller)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.CompletableFuture$Signaller.block(CompletableFuture.java:1864)
        at java.util.concurrent.ForkJoinPool.unmanagedBlock(ForkJoinPool.java:3780)
        at java.util.concurrent.CompletableFuture.waitingGet(CompletableFuture.java:1898)
        at java.util.concurrent.CompletableFuture.get(CompletableFuture.java:2072)
        at FutureLatchDeadlock.lambda$main$0(FutureLatchDeadlock.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"latch-waiter-0" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076bbb0001> (a java.util.concurrent.CountDownLatch$Sync)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer.acquire(AbstractQueuedSynchronizer.java:754)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer.acquireSharedInterruptibly(AbstractQueuedSynchronizer.java:1099)
        at java.util.concurrent.CountDownLatch.await(CountDownLatch.java:230)
        at FutureLatchDeadlock.lambda$main$1(FutureLatchDeadlock.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"latch-waiter-1" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting on condition [0x4]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076bbb0002> (a java.util.concurrent.CountDownLatch$Sync)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer.acquire(AbstractQueuedSynchronizer.java:754)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer.acquireSharedInterruptibly(AbstractQueuedSynchronizer.java:1099)
        at java.util.concurrent.CountDownLatch.await(CountDownLatch.java:230)
        at FutureLatchDeadlock.lambda$main$2(FutureLatchDeadlock.java:50)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_future_latch_wait_tree_pattern() {
        let a = analyze(FUTURE_LATCH_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FutureLatchWaitTree)
            .expect("future-latch-wait-tree");
        assert!(hit.thread_names.len() >= 4, "names={:?}", hit.thread_names);
        assert_eq!(hit.severity, "critical");
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("future-waiter-")),
            "names={:?}",
            hit.thread_names
        );
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("latch-waiter-")),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn single_future_get_is_not_wait_tree() {
        const ONE: &str = r#""worker" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at java.util.concurrent.CompletableFuture.get(CompletableFuture.java:2072)
        at com.example.App.run(App.java:10)
"#;
        let a = analyze(ONE);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::FutureLatchWaitTree)
        );
    }

    const LOGGING_APPENDER_SAMPLE: &str = r#"2026-07-24 15:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"log-holder" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076abc1000> (a LoggingAppenderContention$OutputStreamAppender)
        at LoggingAppenderContention$OutputStreamAppender.append(LoggingAppenderContention.java:18)
        at LoggingAppenderContention$OutputStreamAppender.doAppend(LoggingAppenderContention.java:25)
        at LoggingAppenderContention$Logger.info(LoggingAppenderContention.java:36)
        at LoggingAppenderContention.lambda$main$0(LoggingAppenderContention.java:48)
        at java.lang.Thread.run(Thread.java:1583)

"log-writer-0" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at LoggingAppenderContention$OutputStreamAppender.append(LoggingAppenderContention.java:16)
        - waiting to lock <0x000000076abc1000> (a LoggingAppenderContention$OutputStreamAppender)
        at LoggingAppenderContention$OutputStreamAppender.doAppend(LoggingAppenderContention.java:25)
        at LoggingAppenderContention$Logger.info(LoggingAppenderContention.java:36)
        at LoggingAppenderContention.lambda$main$1(LoggingAppenderContention.java:58)
        at java.lang.Thread.run(Thread.java:1583)

"log-writer-1" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at LoggingAppenderContention$OutputStreamAppender.append(LoggingAppenderContention.java:16)
        - waiting to lock <0x000000076abc1000> (a LoggingAppenderContention$OutputStreamAppender)
        at LoggingAppenderContention$OutputStreamAppender.doAppend(LoggingAppenderContention.java:25)
        at LoggingAppenderContention$Logger.info(LoggingAppenderContention.java:36)
        at LoggingAppenderContention.lambda$main$1(LoggingAppenderContention.java:58)
        at java.lang.Thread.run(Thread.java:1583)

"log-writer-2" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting for monitor entry [0x4]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at LoggingAppenderContention$OutputStreamAppender.append(LoggingAppenderContention.java:16)
        - waiting to lock <0x000000076abc1000> (a LoggingAppenderContention$OutputStreamAppender)
        at LoggingAppenderContention$OutputStreamAppender.doAppend(LoggingAppenderContention.java:25)
        at LoggingAppenderContention$Logger.info(LoggingAppenderContention.java:36)
        at LoggingAppenderContention.lambda$main$1(LoggingAppenderContention.java:58)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_logging_appender_contention_pattern() {
        let a = analyze(LOGGING_APPENDER_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::LoggingAppenderContention)
            .expect("logging-appender-contention");
        assert!(hit.thread_names.len() >= 3, "names={:?}", hit.thread_names);
        assert!(hit.detail.contains("BLOCKED") || hit.detail.contains("appender") || hit.detail.contains("OutputStreamAppender"));
        assert!(
            hit.thread_names.iter().any(|n| n == "log-holder"),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn two_blocked_loggers_without_third_is_not_logging_contention() {
        const TWO: &str = r#""log-writer-0" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at ch.qos.logback.core.OutputStreamAppender.writeOut(OutputStreamAppender.java:200)
        - waiting to lock <0x000000076abc1000> (a ch.qos.logback.core.OutputStreamAppender)

"log-writer-1" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at ch.qos.logback.core.OutputStreamAppender.writeOut(OutputStreamAppender.java:200)
        - waiting to lock <0x000000076abc1000> (a ch.qos.logback.core.OutputStreamAppender)
"#;
        let a = analyze(TWO);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::LoggingAppenderContention)
        );
    }

    const BUSY_WAIT_SAMPLE: &str = r#"2026-07-24 16:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"spin-worker-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at BusyWaitSpin.spinUntilReady(BusyWaitSpin.java:12)
        at BusyWaitSpin.lambda$main$0(BusyWaitSpin.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"spin-worker-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at BusyWaitSpin.spinUntilReady(BusyWaitSpin.java:12)
        at BusyWaitSpin.lambda$main$0(BusyWaitSpin.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"spin-worker-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 runnable [0x3]
   java.lang.Thread.State: RUNNABLE
        at BusyWaitSpin.spinUntilReady(BusyWaitSpin.java:12)
        at BusyWaitSpin.lambda$main$0(BusyWaitSpin.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"spin-worker-3" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 runnable [0x4]
   java.lang.Thread.State: RUNNABLE
        at BusyWaitSpin.spinUntilReady(BusyWaitSpin.java:12)
        at BusyWaitSpin.lambda$main$0(BusyWaitSpin.java:24)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_busy_wait_spin_hotspot_pattern() {
        let a = analyze(BUSY_WAIT_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::BusyWaitSpinHotspot)
            .expect("busy-wait-spin-hotspot");
        assert_eq!(hit.thread_names.len(), 4);
        assert!(hit.detail.contains("spin") || hit.detail.contains("RUNNABLE"));
        assert!(
            hit.thread_names.iter().all(|n| n.starts_with("spin-worker-")),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn runnable_with_park_is_not_busy_wait() {
        const PARKED: &str = r#""w0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at com.example.App.run(App.java:10)

"w1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at com.example.App.run(App.java:10)

"w2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 runnable [0x3]
   java.lang.Thread.State: RUNNABLE
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at com.example.App.run(App.java:10)
"#;
        let a = analyze(PARKED);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::BusyWaitSpinHotspot)
        );
    }

    const CONDITION_STARVATION_SAMPLE: &str = r#"2026-07-24 17:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"cond-waiter-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076abc2000> (a java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at ConditionStarvation.lambda$main$0(ConditionStarvation.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"cond-waiter-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076abc2000> (a java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at ConditionStarvation.lambda$main$0(ConditionStarvation.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"cond-waiter-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076abc2000> (a java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at ConditionStarvation.lambda$main$0(ConditionStarvation.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"cond-waiter-3" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting on condition [0x4]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        - parking to wait for  <0x000000076abc2000> (a java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at ConditionStarvation.lambda$main$0(ConditionStarvation.java:22)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_condition_park_starvation_pattern() {
        let a = analyze(CONDITION_STARVATION_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::ConditionParkStarvation)
            .expect("condition-park-starvation");
        assert_eq!(hit.thread_names.len(), 4);
        assert!(hit.detail.contains("Condition") || hit.detail.contains("signaler"));
        assert!(
            hit.thread_names.iter().all(|n| n.starts_with("cond-waiter-")),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn condition_waiters_with_signaler_not_starvation() {
        const WITH_SIGNAL: &str = r#""cond-waiter-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at com.example.App.await(App.java:10)

"cond-waiter-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: WAITING (parking)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at com.example.App.await(App.java:10)

"cond-waiter-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: WAITING (parking)
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.await(AbstractQueuedSynchronizer.java:1754)
        at com.example.App.await(App.java:10)

"signaler" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 runnable [0x4]
   java.lang.Thread.State: RUNNABLE
        at java.util.concurrent.locks.AbstractQueuedSynchronizer$ConditionObject.signalAll(AbstractQueuedSynchronizer.java:1800)
        at com.example.App.signal(App.java:20)
"#;
        let a = analyze(WITH_SIGNAL);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::ConditionParkStarvation)
        );
    }

    const LOCK_ORDER_SAMPLE: &str = r#"2026-07-24 18:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"order-ab" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting for monitor entry [0x1]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at LockOrderRisk.lambda$main$0(LockOrderRisk.java:24)
        - waiting to lock <0x000000076ab00002> (a java.lang.Object)
        - locked <0x000000076ab00001> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)

"order-ba" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at LockOrderRisk.lambda$main$1(LockOrderRisk.java:42)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        - locked <0x000000076ab00002> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_lock_order_inconsistency_pattern() {
        let a = analyze(LOCK_ORDER_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::LockOrderInconsistency)
            .expect("lock-order-inconsistency");
        assert_eq!(hit.thread_names.len(), 2);
        assert!(hit.detail.contains("inconsistent") || hit.detail.contains("↔"));
        assert!(hit.thread_names.iter().any(|n| n == "order-ab"));
        assert!(hit.thread_names.iter().any(|n| n == "order-ba"));
        // Classic opposite orders also form a wait-for cycle.
        assert!(!a.deadlocks.is_empty());
    }

    #[test]
    fn one_way_lock_order_is_not_inconsistency() {
        const ONE_WAY: &str = r#""worker" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting for monitor entry [0x1]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.run(App.java:10)
        - waiting to lock <0x000000076ab00002> (a java.lang.Object)
        - locked <0x000000076ab00001> (a java.lang.Object)

"holder" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076ab00002> (a java.lang.Object)
"#;
        let a = analyze(ONE_WAY);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::LockOrderInconsistency)
        );
    }

    const FINALIZER_PRESSURE_SAMPLE: &str = r#"2026-07-24 18:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"app-lock-holder" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076ab00001> (a java.lang.Object)
        at FinalizerPressure.lambda$main$0(FinalizerPressure.java:28)
        at java.lang.Thread.run(Thread.java:1583)

"Finalizer" #3 daemon prio=8 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FinalizerPressure$HeavyFinalizer.finalize(FinalizerPressure.java:14)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at java.lang.System$2.invokeFinalize(System.java:2148)
        at java.lang.ref.Finalizer.runFinalizer(Finalizer.java:96)
        at java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:174)

"app-waiter-0" #22 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FinalizerPressure.lambda$main$1(FinalizerPressure.java:55)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)

"app-waiter-1" #23 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting for monitor entry [0x4]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FinalizerPressure.lambda$main$1(FinalizerPressure.java:55)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at java.lang.Thread.run(Thread.java:1583)

"Reference Handler" #2 daemon prio=10 os_prio=0 tid=0x5 nid=0x35 waiting on condition [0x5]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at java.lang.ref.Reference$ReferenceHandler.run(Reference.java:216)
"#;

    #[test]
    fn detects_finalizer_pressure_pattern() {
        let a = analyze(FINALIZER_PRESSURE_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FinalizerPressure)
            .expect("finalizer-pressure");
        assert!(hit.thread_names.iter().any(|n| n == "Finalizer"));
        assert!(hit.thread_names.iter().any(|n| n == "app-lock-holder"));
        assert!(
            hit.detail.contains("Finalizer") || hit.detail.contains("pressure"),
            "detail={}",
            hit.detail
        );
        assert_eq!(hit.severity, "critical");
    }

    #[test]
    fn idle_finalizer_is_not_pressure() {
        const IDLE: &str = r#""Finalizer" #3 daemon prio=8 os_prio=0 tid=0x1 nid=0x31 in Object.wait() [0x1]
   java.lang.Thread.State: WAITING (on object monitor)
        at java.lang.Object.wait(Native Method)
        at java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:151)
        at java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:172)
        at java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:165)

"app-worker" #21 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at com.example.App.run(App.java:10)
"#;
        let a = analyze(IDLE);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::FinalizerPressure)
        );
    }

    const SLEEP_AS_SCHEDULER_SAMPLE: &str = r#"2026-07-24 18:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"sleep-scheduler-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)
        at SleepAsScheduler.lambda$main$0(SleepAsScheduler.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"sleep-scheduler-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)
        at SleepAsScheduler.lambda$main$0(SleepAsScheduler.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"sleep-scheduler-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)
        at SleepAsScheduler.lambda$main$0(SleepAsScheduler.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"sleep-scheduler-3" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting on condition [0x4]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)
        at SleepAsScheduler.lambda$main$0(SleepAsScheduler.java:22)
        at java.lang.Thread.run(Thread.java:1583)

"Finalizer" #3 daemon prio=8 os_prio=0 tid=0x5 nid=0x35 in Object.wait() [0x5]
   java.lang.Thread.State: WAITING (on object monitor)
        at java.lang.Object.wait(Native Method)
        at java.lang.ref.ReferenceQueue.remove(ReferenceQueue.java:151)
        at java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:165)
"#;

    #[test]
    fn detects_sleep_as_scheduler_pattern() {
        let a = analyze(SLEEP_AS_SCHEDULER_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::SleepAsScheduler)
            .expect("sleep-as-scheduler");
        assert_eq!(hit.thread_names.len(), 4);
        assert!(hit.detail.contains("sleep") || hit.detail.contains("scheduler"));
        assert!(
            hit.thread_names.iter().all(|n| n.starts_with("sleep-scheduler-")),
            "names={:?}",
            hit.thread_names
        );
        assert!(!hit.thread_names.iter().any(|n| n == "Finalizer"));
    }

    #[test]
    fn few_sleepers_or_jvm_noise_not_scheduler_pattern() {
        const FEW: &str = r#""sleep-scheduler-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)

"sleep-scheduler-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at SleepAsScheduler.scheduleNextTick(SleepAsScheduler.java:10)

"Finalizer" #3 daemon prio=8 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        at java.lang.ref.Finalizer$FinalizerThread.run(Finalizer.java:165)
"#;
        let a = analyze(FEW);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::SleepAsScheduler)
        );
    }

    const FRAMEWORK_POOL_SAMPLE: &str = r#"2026-07-24 18:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"http-nio-8080-exec-1" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
        - locked <0x000000076ab00001> (a java.lang.Object)
        at FrameworkPoolSaturation.handleRequest(FrameworkPoolSaturation.java:12)
        at FrameworkPoolSaturation.lambda$main$0(FrameworkPoolSaturation.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"http-nio-8080-exec-2" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FrameworkPoolSaturation.handleRequest(FrameworkPoolSaturation.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at FrameworkPoolSaturation.lambda$main$0(FrameworkPoolSaturation.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"http-nio-8080-exec-3" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FrameworkPoolSaturation.handleRequest(FrameworkPoolSaturation.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at FrameworkPoolSaturation.lambda$main$0(FrameworkPoolSaturation.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"http-nio-8080-exec-4" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 waiting for monitor entry [0x4]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at FrameworkPoolSaturation.handleRequest(FrameworkPoolSaturation.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)
        at FrameworkPoolSaturation.lambda$main$0(FrameworkPoolSaturation.java:24)
        at java.lang.Thread.run(Thread.java:1583)

"qtp1234567890-45" #25 prio=5 os_prio=0 tid=0x5 nid=0x35 waiting on condition [0x5]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:221)
        at org.eclipse.jetty.util.thread.QueuedThreadPool.idleJob(QueuedThreadPool.java:900)
"#;

    #[test]
    fn detects_framework_pool_saturation_pattern() {
        let a = analyze(FRAMEWORK_POOL_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FrameworkPoolSaturation)
            .expect("framework-pool-saturation");
        assert!(hit.thread_names.len() >= 3);
        assert!(hit.detail.contains("tomcat") || hit.detail.contains("framework"));
        assert!(
            hit.thread_names
                .iter()
                .all(|n| n.starts_with("http-nio-8080-exec-")),
            "names={:?}",
            hit.thread_names
        );
        // Idle Jetty worker must not be pulled into the Tomcat cluster.
        assert!(!hit.thread_names.iter().any(|n| n.starts_with("qtp")));
    }

    #[test]
    fn detects_jetty_and_netty_framework_names() {
        const MIXED: &str = r#""qtp111-1" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting for monitor entry [0x1]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.handle(App.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)

"qtp111-2" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting for monitor entry [0x2]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.handle(App.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)

"qtp111-3" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting for monitor entry [0x3]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.handle(App.java:10)
        - waiting to lock <0x000000076ab00001> (a java.lang.Object)

"nioEventLoopGroup-2-1" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 runnable [0x4]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.SelectorImpl.select(SelectorImpl.java:100)
        at io.netty.channel.nio.NioEventLoop.select(NioEventLoop.java:800)
"#;
        let a = analyze(MIXED);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::FrameworkPoolSaturation)
            .expect("jetty framework-pool-saturation");
        assert_eq!(hit.thread_names.len(), 3);
        assert!(hit.detail.contains("jetty"));
        assert!(hit.thread_names.iter().all(|n| n.starts_with("qtp")));
        // Idle Netty selector must not form a saturated cluster alone.
        assert!(!hit.thread_names.iter().any(|n| n.contains("nioEventLoop")));
    }

    #[test]
    fn idle_framework_workers_not_saturated() {
        const IDLE: &str = r#""http-nio-8080-exec-1" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 waiting on condition [0x1]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at org.apache.tomcat.util.threads.TaskQueue.take(TaskQueue.java:100)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)

"http-nio-8080-exec-2" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 waiting on condition [0x2]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at org.apache.tomcat.util.threads.TaskQueue.take(TaskQueue.java:100)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)

"http-nio-8080-exec-3" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 waiting on condition [0x3]
   java.lang.Thread.State: WAITING (parking)
        at jdk.internal.misc.Unsafe.park(Native Method)
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:371)
        at java.util.concurrent.LinkedBlockingQueue.take(LinkedBlockingQueue.java:435)
        at org.apache.tomcat.util.threads.TaskQueue.take(TaskQueue.java:100)
        at java.util.concurrent.ThreadPoolExecutor.getTask(ThreadPoolExecutor.java:1071)
"#;
        let a = analyze(IDLE);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::FrameworkPoolSaturation)
        );
    }

    const DNS_STALL_SAMPLE: &str = r#"2026-07-24 18:00:00
Full thread dump OpenJDK 64-Bit Server VM:

"dns-resolver-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.EPoll.wait(Native Method)
        at sun.nio.ch.EPollSelectorImpl.doSelect(EPollSelectorImpl.java:121)
        at com.sun.jndi.dns.DnsClient.blockingReceive(DnsClient.java:545)
        at com.sun.jndi.dns.DnsClient.doUdpQuery(DnsClient.java:509)
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at com.sun.jndi.dns.Resolver.query(Resolver.java:81)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)
        at DnsResolutionStall.lambda$main$0(DnsResolutionStall.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"dns-resolver-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.EPoll.wait(Native Method)
        at sun.nio.ch.EPollSelectorImpl.doSelect(EPollSelectorImpl.java:121)
        at com.sun.jndi.dns.DnsClient.blockingReceive(DnsClient.java:545)
        at com.sun.jndi.dns.DnsClient.doUdpQuery(DnsClient.java:509)
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at com.sun.jndi.dns.Resolver.query(Resolver.java:81)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)
        at DnsResolutionStall.lambda$main$0(DnsResolutionStall.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"dns-resolver-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 runnable [0x3]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.EPoll.wait(Native Method)
        at sun.nio.ch.EPollSelectorImpl.doSelect(EPollSelectorImpl.java:121)
        at com.sun.jndi.dns.DnsClient.blockingReceive(DnsClient.java:545)
        at com.sun.jndi.dns.DnsClient.doUdpQuery(DnsClient.java:509)
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at com.sun.jndi.dns.Resolver.query(Resolver.java:81)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)
        at DnsResolutionStall.lambda$main$0(DnsResolutionStall.java:40)
        at java.lang.Thread.run(Thread.java:1583)

"dns-resolver-3" #24 prio=5 os_prio=0 tid=0x4 nid=0x34 runnable [0x4]
   java.lang.Thread.State: RUNNABLE
        at sun.nio.ch.EPoll.wait(Native Method)
        at sun.nio.ch.EPollSelectorImpl.doSelect(EPollSelectorImpl.java:121)
        at com.sun.jndi.dns.DnsClient.blockingReceive(DnsClient.java:545)
        at com.sun.jndi.dns.DnsClient.doUdpQuery(DnsClient.java:509)
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at com.sun.jndi.dns.Resolver.query(Resolver.java:81)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)
        at DnsResolutionStall.lambda$main$0(DnsResolutionStall.java:40)
        at java.lang.Thread.run(Thread.java:1583)
"#;

    #[test]
    fn detects_dns_resolution_stall_pattern() {
        let a = analyze(DNS_STALL_SAMPLE);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::DnsResolutionStall)
            .expect("dns-resolution-stall");
        assert_eq!(hit.thread_names.len(), 4);
        assert!(hit.detail.contains("DNS") || hit.detail.contains("name-resolution"));
        assert!(
            hit.thread_names.iter().all(|n| n.starts_with("dns-resolver-")),
            "names={:?}",
            hit.thread_names
        );
    }

    #[test]
    fn detects_inetaddress_name_service_stall() {
        const INET: &str = r#""lookup-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at java.net.Inet6AddressImpl.lookupAllHostAddr(Native Method)
        at java.net.InetAddress$PlatformNameService.lookupAllHostAddr(InetAddress.java:929)
        at java.net.InetAddress.getAddressesFromNameService(InetAddress.java:1515)
        at java.net.InetAddress$NameServiceAddresses.get(InetAddress.java:848)
        at java.net.InetAddress.getAllByName0(InetAddress.java:1505)
        at java.net.InetAddress.getAllByName(InetAddress.java:1364)
        at java.net.InetAddress.getByName(InetAddress.java:1315)
        at com.example.App.connect(App.java:10)

"lookup-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at java.net.Inet6AddressImpl.lookupAllHostAddr(Native Method)
        at java.net.InetAddress$PlatformNameService.lookupAllHostAddr(InetAddress.java:929)
        at java.net.InetAddress.getAddressesFromNameService(InetAddress.java:1515)
        at java.net.InetAddress$NameServiceAddresses.get(InetAddress.java:848)
        at java.net.InetAddress.getAllByName0(InetAddress.java:1505)
        at java.net.InetAddress.getAllByName(InetAddress.java:1364)
        at java.net.InetAddress.getByName(InetAddress.java:1315)
        at com.example.App.connect(App.java:10)

"lookup-2" #23 prio=5 os_prio=0 tid=0x3 nid=0x33 runnable [0x3]
   java.lang.Thread.State: RUNNABLE
        at java.net.Inet6AddressImpl.lookupAllHostAddr(Native Method)
        at java.net.InetAddress$PlatformNameService.lookupAllHostAddr(InetAddress.java:929)
        at java.net.InetAddress.getAddressesFromNameService(InetAddress.java:1515)
        at java.net.InetAddress$NameServiceAddresses.get(InetAddress.java:848)
        at java.net.InetAddress.getAllByName0(InetAddress.java:1505)
        at java.net.InetAddress.getAllByName(InetAddress.java:1364)
        at java.net.InetAddress.getByName(InetAddress.java:1315)
        at com.example.App.connect(App.java:10)
"#;
        let a = analyze(INET);
        let hit = a
            .patterns
            .iter()
            .find(|p| p.kind == PatternKind::DnsResolutionStall)
            .expect("inetaddress dns-resolution-stall");
        assert_eq!(hit.thread_names.len(), 3);
        assert!(hit.detail.contains("InetAddress") || hit.detail.contains("DNS") || hit.detail.contains("lookup"));
    }

    #[test]
    fn few_dns_threads_not_a_stall_cluster() {
        const FEW: &str = r#""dns-resolver-0" #21 prio=5 os_prio=0 tid=0x1 nid=0x31 runnable [0x1]
   java.lang.Thread.State: RUNNABLE
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)

"dns-resolver-1" #22 prio=5 os_prio=0 tid=0x2 nid=0x32 runnable [0x2]
   java.lang.Thread.State: RUNNABLE
        at com.sun.jndi.dns.DnsClient.query(DnsClient.java:259)
        at DnsResolutionStall.resolveHost(DnsResolutionStall.java:12)
"#;
        let a = analyze(FEW);
        assert!(
            a.patterns
                .iter()
                .all(|p| p.kind != PatternKind::DnsResolutionStall)
        );
    }

    #[test]
    fn detects_thread_leak_across_dumps() {
        let t0 = include_str!("../tests/fixtures/patterns/cross_dump/thread_leak_t0.txt");
        let t1 = include_str!("../tests/fixtures/patterns/cross_dump/thread_leak_t1.txt");
        let t2 = include_str!("../tests/fixtures/patterns/cross_dump/thread_leak_t2.txt");
        let series = analyze_series(&[t0, t1, t2]);
        let hit = series
            .cross_patterns
            .iter()
            .find(|p| p.kind == PatternKind::ThreadLeak)
            .expect("thread-leak");
        assert!(hit.detail.contains("+5") || hit.detail.contains("3 → 5 → 8"));
        assert!(
            hit.thread_names.iter().any(|n| n.starts_with("worker-")),
            "names={:?}",
            hit.thread_names
        );
        assert_eq!(series.dumps.len(), 3);
    }

    #[test]
    fn detects_livelock_across_dumps() {
        let t0 = include_str!("../tests/fixtures/patterns/cross_dump/livelock_t0.txt");
        let t1 = include_str!("../tests/fixtures/patterns/cross_dump/livelock_t1.txt");
        let t2 = include_str!("../tests/fixtures/patterns/cross_dump/livelock_t2.txt");
        let series = analyze_series(&[t0, t1, t2]);
        let hit = series
            .cross_patterns
            .iter()
            .find(|p| p.kind == PatternKind::Livelock)
            .expect("livelock");
        assert_eq!(hit.thread_names.len(), 3);
        assert!(hit.detail.contains("livelock") || hit.detail.contains("changing stacks"));
        assert!(hit.thread_names.iter().all(|n| n.starts_with("spin-")));
    }

    #[test]
    fn stable_series_has_no_cross_patterns() {
        let t0 = include_str!("../tests/fixtures/patterns/cross_dump/stable_t0.txt");
        let t1 = include_str!("../tests/fixtures/patterns/cross_dump/stable_t1.txt");
        let series = analyze_series(&[t0, t1]);
        assert!(
            series.cross_patterns.is_empty(),
            "unexpected {:?}",
            series.cross_patterns
        );
    }

    #[test]
    fn single_dump_series_has_no_cross_patterns() {
        let t0 = include_str!("../tests/fixtures/patterns/cross_dump/thread_leak_t2.txt");
        let series = analyze_series(&[t0]);
        assert!(series.cross_patterns.is_empty());
        assert_eq!(series.dumps.len(), 1);
        assert_eq!(series.dumps[0].total_threads, analyze(t0).total_threads);
    }
}
