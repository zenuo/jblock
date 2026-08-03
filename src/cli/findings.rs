//! Findings-first helpers mirroring web `analysisUi` (feat-056).

use crate::parser::{Analysis, BlockedEdge, PatternHit};

/// Aggregated waiters on one lock, hottest first.
#[derive(Debug, Clone)]
pub struct ContentionGroup {
    pub lock: String,
    pub owner_thread: Option<String>,
    pub waiters: Vec<String>,
}

/// One CLI finding row.
#[derive(Debug, Clone)]
pub struct CliFinding {
    /// `critical` | `warning` | `info`
    pub severity: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
}

/// Group blocked edges by lock id (hottest first).
pub fn aggregate_contention(edges: &[BlockedEdge]) -> Vec<ContentionGroup> {
    let mut map: Vec<(String, ContentionGroup)> = Vec::new();
    for e in edges {
        if let Some((_, g)) = map.iter_mut().find(|(k, _)| k == &e.lock) {
            if g.owner_thread.is_none() {
                g.owner_thread = e.owner_thread.clone();
            }
            if !g.waiters.iter().any(|w| w == &e.blocked_thread) {
                g.waiters.push(e.blocked_thread.clone());
            }
        } else {
            map.push((
                e.lock.clone(),
                ContentionGroup {
                    lock: e.lock.clone(),
                    owner_thread: e.owner_thread.clone(),
                    waiters: vec![e.blocked_thread.clone()],
                },
            ));
        }
    }
    let mut groups: Vec<ContentionGroup> = map.into_iter().map(|(_, g)| g).collect();
    groups.sort_by(|a, b| b.waiters.len().cmp(&a.waiters.len()));
    groups
}

/// Build actionable findings (deadlocks, patterns, hot lock, blocked summary).
/// Does not emit a "clean" placeholder (feat-054 parity).
pub fn build_cli_findings(analysis: &Analysis) -> Vec<CliFinding> {
    let mut findings = Vec::new();
    let blocked = analysis
        .state_counts
        .iter()
        .find(|s| s.state == "BLOCKED")
        .map(|s| s.count)
        .unwrap_or(0);
    let blocked_pct = if analysis.total_threads == 0 {
        0
    } else {
        (blocked * 100) / analysis.total_threads
    };

    for d in &analysis.deadlocks {
        let cycle = if d.threads.is_empty() {
            String::new()
        } else {
            format!("{} → {}", d.threads.join(" → "), d.threads[0])
        };
        findings.push(CliFinding {
            severity: "critical".into(),
            kind: "deadlock".into(),
            title: format!("Deadlock ({} threads)", d.threads.len()),
            detail: cycle,
        });
    }

    for p in &analysis.patterns {
        findings.push(pattern_finding(p));
    }

    let groups = aggregate_contention(&analysis.blocked_edges);
    if let Some(hot) = groups.first() {
        let severity = if analysis.deadlocks.is_empty() {
            "critical"
        } else {
            "warning"
        };
        findings.push(CliFinding {
            severity: severity.into(),
            kind: "hot-lock".into(),
            title: format!("Hot lock ({} waiters)", hot.waiters.len()),
            detail: format!(
                "{} · owner={}",
                hot.lock,
                hot.owner_thread.as_deref().unwrap_or("unknown")
            ),
        });
    }

    if blocked > 0 {
        let severity = if blocked_pct >= 20 { "warning" } else { "info" };
        findings.push(CliFinding {
            severity: severity.into(),
            kind: "blocked".into(),
            title: format!("{blocked} BLOCKED ({blocked_pct}%)"),
            detail: format!("{} contention edge(s)", analysis.blocked_edges.len()),
        });
    }

    findings
}

fn pattern_finding(p: &PatternHit) -> CliFinding {
    let kind = pattern_kind_label(p);
    let title = format!(
        "{} ({} threads)",
        kind_title(kind),
        p.thread_names.len()
    );
    CliFinding {
        severity: p.severity.clone(),
        kind: kind.to_string(),
        title,
        detail: p.detail.clone(),
    }
}

fn pattern_kind_label(p: &PatternHit) -> &'static str {
    // Match serde kebab-case used by WASM / web types.
    use crate::parser::PatternKind::*;
    match p.kind {
        ThreadPoolExhaustion => "thread-pool-exhaustion",
        SyncIoHotspot => "sync-io-hotspot",
        DangerousHotLockOwner => "dangerous-hot-lock-owner",
        ConnectionPoolBorrow => "connection-pool-borrow",
        FutureLatchWaitTree => "future-latch-wait-tree",
        LoggingAppenderContention => "logging-appender-contention",
        BusyWaitSpinHotspot => "busy-wait-spin-hotspot",
        ConditionParkStarvation => "condition-park-starvation",
        LockOrderInconsistency => "lock-order-inconsistency",
        FinalizerPressure => "finalizer-pressure",
        SleepAsScheduler => "sleep-as-scheduler",
        FrameworkPoolSaturation => "framework-pool-saturation",
        DnsResolutionStall => "dns-resolution-stall",
        ThreadLeak => "thread-leak",
        Livelock => "livelock",
    }
}

fn kind_title(kind: &str) -> String {
    kind.replace('-', " ")
}

/// HotSpot / JDK system thread names (web `isJvmNoise` parity).
pub fn is_jvm_noise(name: &str) -> bool {
    let n = name.to_lowercase();
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
    if EXACT.iter().any(|e| *e == n) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::analyze;

    #[test]
    fn deadlock_fixture_emits_deadlock_finding() {
        let a = analyze(include_str!("../../tests/fixtures/deadlock_real_jstack.txt"));
        let findings = build_cli_findings(&a);
        assert!(
            findings.iter().any(|f| f.kind == "deadlock"),
            "{findings:?}"
        );
    }

    #[test]
    fn aggregate_groups_by_lock() {
        let edges = vec![
            BlockedEdge {
                blocked_thread: "a".into(),
                lock: "L1".into(),
                owner_thread: Some("owner".into()),
            },
            BlockedEdge {
                blocked_thread: "b".into(),
                lock: "L1".into(),
                owner_thread: Some("owner".into()),
            },
            BlockedEdge {
                blocked_thread: "c".into(),
                lock: "L2".into(),
                owner_thread: None,
            },
        ];
        let g = aggregate_contention(&edges);
        assert_eq!(g[0].lock, "L1");
        assert_eq!(g[0].waiters.len(), 2);
    }

    #[test]
    fn jvm_noise_detects_finalizer() {
        assert!(is_jvm_noise("Finalizer"));
        assert!(!is_jvm_noise("http-nio-8080-exec-1"));
    }
}
