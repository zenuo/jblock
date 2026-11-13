//! GC findings (feat-072). Rules GC01–GC14 and their sources are documented in
//! `docs/design/gc-log-analysis.md` §4.2: JDK Mission Control rules, JVM
//! defaults (`MaxGCPauseMillis`, `GCTimeRatio`, `GCTimeLimit`,
//! `GCHeapFreeLimit`), Oracle tuning guides and GCeasy patterns.
//!
//! `critical` is reserved for consecutive Full GCs that leave < 2% free,
//! heap free < 2% after GC, G1 evacuation failure and CMS failures.

use serde::Serialize;

use super::model::{Collector, GcEvent, GcKind, MemChange, ParsedGcLog};
use super::stats::GcStats;
use super::units::small_sort_by;

/// One problem-first finding with evidence and time anchors.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcFinding {
    /// Rule id, e.g. `GC01`.
    pub rule: String,
    /// `critical` | `warning` | `info`
    pub severity: String,
    /// Machine-readable evidence (`key=value;…`) for tests, CLI and i18n rendering.
    pub detail: String,
    /// Time ranges (ms on the analysis axis) the finding points at.
    pub anchors: Vec<(f64, f64)>,
}

/// Default thresholds (see the design doc for each source).
#[derive(Debug, Clone, PartialEq)]
pub struct GcThresholds {
    /// G1 `MaxGCPauseMillis` default when the log does not state a target.
    pub g1_pause_target_ms: f64,
    /// JMC `LongGcPauseRule` info limit, used as the warning bar here.
    pub long_pause_ms: f64,
    /// JMC `GcPauseRatioRule` window and limits.
    pub window_ms: f64,
    pub window_info_pct: f64,
    pub window_warning_pct: f64,
    /// JVM `GCTimeLimit`.
    pub window_critical_pct: f64,
    pub consecutive_full_min: usize,
    /// JVM `GCHeapFreeLimit`.
    pub heap_free_limit_pct: f64,
    /// JMC `SystemGcRule` ratio for WARNING.
    pub system_gc_warning_ratio: f64,
    pub humongous_cause_pct: f64,
    pub humongous_vs_old_pct: f64,
    pub heap_after_warning_pct: f64,
    /// JMC `IncreasingMetaspaceLiveSetRule` slope for WARNING (MiB/s).
    pub metaspace_slope_mib_s: f64,
    pub metadata_threshold_min: usize,
    pub warmup_uptime_ms: f64,
    pub warmup_fraction: f64,
    pub leak_min_points: usize,
    pub leak_min_span_ms: f64,
    pub leak_min_r2: f64,
    /// JMC `IncreasingLiveSetRule`: 1% of the post-warmup live set per second → WARNING.
    pub leak_warning_pct_per_s: f64,
    pub leak_exhaust_hours: f64,
    pub cpu_real_factor: f64,
    pub cpu_real_min_ms: f64,
    pub young_interval_ms: f64,
    /// JMC `ZGCAllocationStallRule` per-minute limits.
    pub stall_info_per_min: f64,
    pub stall_warning_per_min: f64,
}

impl Default for GcThresholds {
    fn default() -> Self {
        Self {
            g1_pause_target_ms: 200.0,
            long_pause_ms: 1000.0,
            window_ms: 60_000.0,
            window_info_pct: 5.0,
            window_warning_pct: 10.0,
            window_critical_pct: 98.0,
            consecutive_full_min: 3,
            heap_free_limit_pct: 2.0,
            system_gc_warning_ratio: 0.5,
            humongous_cause_pct: 10.0,
            humongous_vs_old_pct: 50.0,
            heap_after_warning_pct: 85.0,
            metaspace_slope_mib_s: 0.75,
            metadata_threshold_min: 3,
            warmup_uptime_ms: 5.0 * 60_000.0,
            warmup_fraction: 0.10,
            leak_min_points: 5,
            leak_min_span_ms: 30.0 * 60_000.0,
            leak_min_r2: 0.7,
            leak_warning_pct_per_s: 1.0,
            leak_exhaust_hours: 24.0,
            cpu_real_factor: 1.1,
            cpu_real_min_ms: 100.0,
            young_interval_ms: 1000.0,
            stall_info_per_min: 10.0,
            stall_warning_per_min: 100.0,
        }
    }
}

/// Maximum anchors kept per finding.
const MAX_ANCHORS: usize = 50;

/// Causes that mean an explicit, user- or tool-requested collection.
fn is_explicit_cause(cause: Option<&str>) -> bool {
    matches!(
        cause,
        Some("System.gc()")
            | Some("Diagnostic Command")
            | Some("Heap Dump Initiated GC")
            | Some("Heap Inspection Initiated GC")
            | Some("WhiteBox Initiated Young GC")
            | Some("WhiteBox Initiated Full GC")
    )
}

fn anchor(e: &GcEvent) -> (f64, f64) {
    (e.time_ms, e.time_ms + e.duration_ms)
}

fn anchors<'a>(events: impl Iterator<Item = &'a GcEvent>) -> Vec<(f64, f64)> {
    events.take(MAX_ANCHORS).map(anchor).collect()
}

fn finding(rule: &str, severity: &str, detail: String, anchors: Vec<(f64, f64)>) -> GcFinding {
    GcFinding { rule: rule.into(), severity: severity.into(), detail, anchors }
}

fn round(v: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (v * f).round() / f
}

/// Old generation when logged (with capacity), else the whole heap.
fn old_or_heap(e: &GcEvent) -> Option<MemChange> {
    match e.old {
        Some(o) if o.capacity_after_kb.is_some() => Some(o),
        _ => e.heap.or(e.old),
    }
}

/// Least-squares fit `y = a + b·x`; returns `(a, b, r²)`.
pub fn linear_fit(points: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = points.len() as f64;
    if points.len() < 2 {
        return None;
    }
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let syy: f64 = points.iter().map(|p| (p.1 - my).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let b = sxy / sxx;
    let a = my - b * mx;
    let r2 = if syy == 0.0 { 0.0 } else { (sxy * sxy) / (sxx * syy) };
    Some((a, b, r2))
}

struct Ctx<'a> {
    log: &'a ParsedGcLog,
    stats: &'a GcStats,
    th: &'a GcThresholds,
    pauses: Vec<&'a GcEvent>,
    /// Events that report the heap: pauses, plus ZGC / Shenandoah cycles.
    memory: Vec<&'a GcEvent>,
}

/// GC01: Full GC on collectors where it is a failure mode (JMC `FullGcRule`: G1 / CMS).
fn full_gc(c: &Ctx) -> Option<GcFinding> {
    if !matches!(c.log.collector, Collector::G1 | Collector::Cms | Collector::Shenandoah) {
        return None;
    }
    let fulls: Vec<&GcEvent> =
        c.pauses.iter().copied().filter(|e| e.kind == GcKind::Full && !is_explicit_cause(e.cause.as_deref())).collect();
    if fulls.is_empty() {
        return None;
    }
    let mut causes: Vec<(String, usize)> = Vec::new();
    for e in &fulls {
        let cause = e.cause.clone().unwrap_or_else(|| e.name.clone());
        match causes.iter_mut().find(|(c, _)| *c == cause) {
            Some((_, n)) => *n += 1,
            None => causes.push((cause, 1)),
        }
    }
    let max = fulls.iter().map(|e| e.duration_ms).fold(0.0, f64::max);
    let causes = causes.iter().map(|(c, n)| format!("{c}:{n}")).collect::<Vec<_>>().join(",");
    Some(finding(
        "GC01",
        "warning",
        format!("count={};max_ms={};causes={causes}", fulls.len(), round(max, 3)),
        anchors(fulls.into_iter()),
    ))
}

/// GC02: back-to-back Full GCs (GCeasy); critical when each leaves < GCHeapFreeLimit free or frees < 2%.
fn consecutive_full(c: &Ctx) -> Option<GcFinding> {
    let mut runs: Vec<Vec<&GcEvent>> = Vec::new();
    let mut cur: Vec<&GcEvent> = Vec::new();
    for e in &c.pauses {
        match e.kind {
            GcKind::Full => cur.push(e),
            GcKind::Young | GcKind::Mixed => {
                if cur.len() >= c.th.consecutive_full_min {
                    runs.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
            _ => {}
        }
    }
    if cur.len() >= c.th.consecutive_full_min {
        runs.push(cur);
    }
    if runs.is_empty() {
        return None;
    }
    let starved = |e: &&GcEvent| {
        let Some(m) = old_or_heap(e) else { return false };
        let Some(cap) = m.capacity_after_kb.filter(|c| *c > 0.0) else { return false };
        let free_pct = 100.0 * (cap - m.after_kb) / cap;
        let freed_pct = 100.0 * (m.before_kb - m.after_kb) / cap;
        free_pct < c.th.heap_free_limit_pct || freed_pct < c.th.heap_free_limit_pct
    };
    let critical_runs = runs.iter().filter(|r| r.iter().all(starved)).count();
    let longest = runs.iter().map(Vec::len).max().unwrap_or(0);
    let total: usize = runs.iter().map(Vec::len).sum();
    let max = runs.iter().flatten().map(|e| e.duration_ms).fold(0.0, f64::max);
    let anchors = runs
        .iter()
        .take(MAX_ANCHORS)
        .map(|r| (r[0].time_ms, r[r.len() - 1].time_ms + r[r.len() - 1].duration_ms))
        .collect();
    Some(finding(
        "GC02",
        if critical_runs > 0 { "critical" } else { "warning" },
        format!("runs={};longest={longest};full={total};starved_runs={critical_runs};max_ms={}", runs.len(), round(max, 3)),
        anchors,
    ))
}

/// GC03: explicit System.gc() (JMC `SystemGcRule`), RMI DGC hint when ~hourly.
fn system_gc(c: &Ctx) -> Option<GcFinding> {
    let sys: Vec<&GcEvent> = c.pauses.iter().copied().filter(|e| e.cause.as_deref() == Some("System.gc()")).collect();
    if sys.is_empty() {
        return None;
    }
    let ratio = sys.len() as f64 / c.pauses.len().max(1) as f64;
    let hourly = sys.len() >= 3
        && sys.windows(2).all(|w| {
            let gap = w[1].time_ms - w[0].time_ms;
            (gap - 3_600_000.0).abs() <= 180_000.0
        });
    Some(finding(
        "GC03",
        if ratio >= c.th.system_gc_warning_ratio { "warning" } else { "info" },
        format!("count={};ratio_pct={};hourly={}", sys.len(), round(ratio * 100.0, 1), hourly as u8),
        anchors(sys.into_iter()),
    ))
}

/// GC04: G1 evacuation failure / to-space exhausted.
fn evacuation_failure(c: &Ctx) -> Option<GcFinding> {
    let ev: Vec<&GcEvent> = c.pauses.iter().copied().filter(|e| e.flags.evacuation_failure).collect();
    (!ev.is_empty()).then(|| finding("GC04", "critical", format!("count={}", ev.len()), anchors(ev.into_iter())))
}

/// GC05: CMS promotion failed / concurrent mode failure.
fn cms_failure(c: &Ctx) -> Option<GcFinding> {
    let promo = c.log.events.iter().filter(|e| e.flags.promotion_failed).count();
    let cmf = c.log.events.iter().filter(|e| e.flags.concurrent_mode_failure).count();
    if promo + cmf == 0 {
        return None;
    }
    let ev = c.log.events.iter().filter(|e| e.flags.promotion_failed || e.flags.concurrent_mode_failure);
    Some(finding("GC05", "critical", format!("promotion_failed={promo};concurrent_mode_failure={cmf}"), anchors(ev)))
}

/// GC06: frequent humongous allocation (G1).
fn humongous(c: &Ctx) -> Option<GcFinding> {
    if c.log.collector != Collector::G1 || c.pauses.is_empty() {
        return None;
    }
    let hum: Vec<&GcEvent> =
        c.pauses.iter().copied().filter(|e| e.cause.as_deref() == Some("G1 Humongous Allocation")).collect();
    let cause_pct = 100.0 * hum.len() as f64 / c.pauses.len() as f64;
    let peak_vs_old = c
        .pauses
        .iter()
        .filter_map(|e| {
            let h = e.humongous?;
            let o = e.old?;
            // A tiny warm-up old gen makes the ratio meaningless: require old ≥ 10% of the heap.
            let cap = e.heap?.capacity_after_kb?;
            (o.before_kb >= 0.1 * cap && o.before_kb > 0.0).then(|| 100.0 * h.before_kb / o.before_kb)
        })
        .fold(0.0, f64::max);
    if cause_pct <= c.th.humongous_cause_pct && peak_vs_old <= c.th.humongous_vs_old_pct {
        return None;
    }
    Some(finding(
        "GC06",
        "warning",
        format!("count={};cause_pct={};peak_vs_old_pct={}", hum.len(), round(cause_pct, 1), round(peak_vs_old, 1)),
        anchors(hum.into_iter()),
    ))
}

/// GC07: long pauses vs the pause target (JMC `LongGcPauseRule`: 1 s is only INFO there).
fn long_pauses(c: &Ctx) -> Option<GcFinding> {
    let p = c.stats.pause?;
    let target = c.log.jvm.pause_target_ms.or((c.log.collector == Collector::G1).then_some(c.th.g1_pause_target_ms));
    let over_target = target.is_some_and(|t| p.max > t);
    let warning = target.is_some_and(|t| p.p99 > t) || p.max > c.th.long_pause_ms;
    if !(over_target || warning) {
        return None;
    }
    let bar = target.unwrap_or(c.th.long_pause_ms).min(c.th.long_pause_ms);
    // Ten longest, without sorting every pause above the bar.
    let mut longest: Vec<&GcEvent> = Vec::with_capacity(11);
    for e in c.pauses.iter().copied().filter(|e| e.duration_ms > bar) {
        let pos = longest.iter().position(|x| e.duration_ms > x.duration_ms).unwrap_or(longest.len());
        if pos < 10 {
            longest.insert(pos, e);
            longest.truncate(10);
        }
    }
    let count_over = c.pauses.iter().filter(|e| e.duration_ms > bar).count();
    Some(finding(
        "GC07",
        if warning { "warning" } else { "info" },
        format!(
            "max_ms={};p99_ms={};target_ms={};count_over={count_over}",
            round(p.max, 3),
            round(p.p99, 3),
            target.map_or("none".to_string(), |t| t.to_string())
        ),
        anchors(longest.into_iter()),
    ))
}

/// Collector's own GC-time goal `1 / (1 + GCTimeRatio)` in percent (G1 12, Parallel 99).
fn gc_time_goal_pct(collector: Collector) -> Option<f64> {
    match collector {
        Collector::G1 => Some(100.0 / 13.0),
        Collector::Parallel => Some(1.0),
        _ => None,
    }
}

/// GC08: GC pause ratio in a sliding window (JMC `GcPauseRatioRule`) + collector goal.
fn pause_ratio(c: &Ctx) -> Option<GcFinding> {
    if c.pauses.is_empty() || c.stats.span_ms <= 0.0 {
        return None;
    }
    let window = c.th.window_ms.min(c.stats.span_ms);
    let (mut best, mut best_at) = (0.0f64, (0.0, 0.0));
    let mut j = 0;
    let mut sum = 0.0;
    for i in 0..c.pauses.len() {
        let start = c.pauses[i].time_ms;
        while j < c.pauses.len() && c.pauses[j].time_ms < start + window {
            sum += c.pauses[j].duration_ms;
            j += 1;
        }
        let pct = (100.0 * sum / window).min(100.0);
        if pct > best {
            best = pct;
            best_at = (start, start + window);
        }
        sum -= c.pauses[i].duration_ms;
    }
    let global = 100.0 - c.stats.throughput_pct.unwrap_or(100.0);
    let goal = gc_time_goal_pct(c.log.collector);
    let over_goal = goal.is_some_and(|g| global > g);
    let severity = if best >= c.th.window_critical_pct {
        "critical"
    } else if best > c.th.window_warning_pct {
        "warning"
    } else if best > c.th.window_info_pct || over_goal {
        "info"
    } else {
        return None;
    };
    Some(finding(
        "GC08",
        severity,
        format!(
            "window_pct={};window_s={};global_pct={};goal_pct={}",
            round(best, 2),
            round(window / 1000.0, 1),
            round(global, 2),
            goal.map_or("none".to_string(), |g| round(g, 2).to_string())
        ),
        vec![best_at],
    ))
}

/// GC09: live-set growth (JMC `IncreasingLiveSetRule`) with warm-up exclusion and R² noise filter.
fn leak_trend(c: &Ctx) -> Option<GcFinding> {
    let series: Vec<(f64, f64)> = c
        .memory
        .iter()
        .filter_map(|e| e.old.map(|o| o.after_kb).or(e.heap.map(|h| h.after_kb)).map(|v| (e.time_ms, v)))
        .collect();
    let (first, last) = (series.first()?.0, series.last()?.0);
    let has_uptime = c.log.events.iter().any(|e| e.uptime_ms.is_some());
    let mut cut = first + c.th.warmup_fraction * (last - first);
    if has_uptime {
        cut = cut.max(c.th.warmup_uptime_ms);
    }
    let points: Vec<(f64, f64)> = series.into_iter().filter(|p| p.0 >= cut).collect();
    if points.len() < c.th.leak_min_points {
        return None;
    }
    let span = points.last()?.0 - points[0].0;
    if span < c.th.leak_min_span_ms {
        return None;
    }
    let (a, slope_kb_per_ms, r2) = linear_fit(&points)?;
    if slope_kb_per_ms <= 0.0 || r2 < c.th.leak_min_r2 {
        return None;
    }
    let base = points[0].1.max(1.0);
    let pct_per_s = 100.0 * slope_kb_per_ms * 1000.0 / base;
    let now = a + slope_kb_per_ms * points.last()?.0;
    let hours_to_full = c.log.jvm.heap_max_kb.map(|max| ((max - now).max(0.0) / slope_kb_per_ms) / 3_600_000.0);
    let has_old_gc = c.pauses.iter().any(|e| matches!(e.kind, GcKind::Full | GcKind::Mixed | GcKind::Remark));
    let warning = pct_per_s >= c.th.leak_warning_pct_per_s || hours_to_full.is_some_and(|h| h < c.th.leak_exhaust_hours);
    Some(finding(
        "GC09",
        if warning && has_old_gc { "warning" } else { "info" },
        format!(
            "slope_mb_per_h={};pct_per_s={};r2={};hours_to_full={};points={};old_gc={}",
            round(slope_kb_per_ms * 3_600_000.0 / 1024.0, 2),
            round(pct_per_s, 4),
            round(r2, 3),
            hours_to_full.map_or("none".to_string(), |h| round(h, 1).to_string()),
            points.len(),
            has_old_gc as u8
        ),
        vec![(points[0].0, points.last()?.0)],
    ))
}

/// GC10: heap still nearly full after GC; ≥ 98% (free < GCHeapFreeLimit) is critical.
fn heap_after_high(c: &Ctx) -> Option<GcFinding> {
    let mut peak: Option<(&GcEvent, f64)> = None;
    for e in &c.memory {
        let Some(h) = e.heap else { continue };
        let Some(denom) = c.log.jvm.heap_max_kb.or(h.capacity_after_kb).filter(|d| *d > 0.0) else { continue };
        let pct = 100.0 * h.after_kb / denom;
        if peak.is_none_or(|(_, p)| pct > p) {
            peak = Some((e, pct));
        }
    }
    let (e, pct) = peak?;
    let critical_bar = 100.0 - c.th.heap_free_limit_pct;
    if pct <= c.th.heap_after_warning_pct {
        return None;
    }
    let over: Vec<&GcEvent> = c
        .memory
        .iter()
        .copied()
        .filter(|x| {
            x.heap.zip(c.log.jvm.heap_max_kb.or(x.heap.and_then(|h| h.capacity_after_kb))).is_some_and(|(h, d)| {
                d > 0.0 && 100.0 * h.after_kb / d > c.th.heap_after_warning_pct
            })
        })
        .collect();
    let mut a = anchors(over.iter().copied());
    if a.is_empty() {
        a.push(anchor(e));
    }
    Some(finding(
        "GC10",
        if pct >= critical_bar { "critical" } else { "warning" },
        format!("peak_pct={};count_over={}", round(pct, 1), over.len()),
        a,
    ))
}

/// GC11: metaspace growth (JMC `IncreasingMetaspaceLiveSetRule`), Metadata GC Threshold as evidence.
fn metaspace(c: &Ctx) -> Option<GcFinding> {
    // Metaspace grows while classes load at start-up: only count threshold GCs after warm-up.
    let has_uptime = c.log.events.iter().any(|e| e.uptime_ms.is_some());
    let threshold: Vec<&GcEvent> = c
        .pauses
        .iter()
        .copied()
        .filter(|e| e.cause.as_deref() == Some("Metadata GC Threshold") && (!has_uptime || e.time_ms >= c.th.warmup_uptime_ms))
        .collect();
    let pts: Vec<(f64, f64)> = c.pauses.iter().filter_map(|e| e.metaspace.map(|m| (e.time_ms, m.after_kb))).collect();
    let cut = pts.first().map(|p| p.0).unwrap_or(0.0).max(c.th.warmup_uptime_ms);
    let post: Vec<(f64, f64)> = pts.into_iter().filter(|p| p.0 >= cut).collect();
    let slope_mib_s = if post.len() >= c.th.leak_min_points {
        linear_fit(&post).map(|(_, b, _)| b * 1000.0 / 1024.0).unwrap_or(0.0)
    } else {
        0.0
    };
    let severity = if slope_mib_s >= c.th.metaspace_slope_mib_s {
        "warning"
    } else if threshold.len() >= c.th.metadata_threshold_min {
        "info"
    } else {
        return None;
    };
    Some(finding(
        "GC11",
        severity,
        format!("slope_mib_s={};metadata_threshold={}", round(slope_mib_s, 3), threshold.len()),
        anchors(threshold.into_iter()),
    ))
}

/// GC12: real time well above user+sys (JMC `GcInvertedParallelismRule`); sys > user as info.
fn parallelism(c: &Ctx) -> Option<GcFinding> {
    let inverted: Vec<&GcEvent> = c
        .pauses
        .iter()
        .copied()
        .filter(|e| {
            e.cpu.is_some_and(|cpu| cpu.real_ms > c.th.cpu_real_min_ms && cpu.real_ms > (cpu.user_ms + cpu.sys_ms) * c.th.cpu_real_factor)
        })
        .collect();
    let sys_heavy = c
        .pauses
        .iter()
        .filter(|e| e.cpu.is_some_and(|cpu| cpu.sys_ms > cpu.user_ms && cpu.sys_ms + cpu.user_ms >= 10.0))
        .count();
    if inverted.is_empty() && sys_heavy == 0 {
        return None;
    }
    let severity = if inverted.is_empty() { "info" } else { "warning" };
    let a = if inverted.is_empty() {
        anchors(c.pauses.iter().copied().filter(|e| e.cpu.is_some_and(|cpu| cpu.sys_ms > cpu.user_ms && cpu.sys_ms + cpu.user_ms >= 10.0)))
    } else {
        anchors(inverted.iter().copied())
    };
    Some(finding("GC12", severity, format!("inverted={};sys_heavy={sys_heavy}", inverted.len()), a))
}

/// GC13: young collections more often than once a second on average.
fn frequent_young(c: &Ctx) -> Option<GcFinding> {
    let young: Vec<&GcEvent> = c.pauses.iter().copied().filter(|e| e.kind == GcKind::Young).collect();
    if young.len() < 10 {
        return None;
    }
    let mean = (young.last()?.time_ms - young[0].time_ms) / (young.len() - 1) as f64;
    (mean < c.th.young_interval_ms).then(|| {
        finding(
            "GC13",
            "info",
            format!("mean_interval_ms={};count={}", round(mean, 1), young.len()),
            vec![(young[0].time_ms, young.last().map_or(0.0, |e| e.time_ms))],
        )
    })
}

/// GC14: ZGC allocation stalls (JMC per-minute limits) and Shenandoah degenerated GCs.
fn concurrent_degradation(c: &Ctx) -> Option<GcFinding> {
    let stalls: Vec<&GcEvent> = c.log.events.iter().filter(|e| e.flags.allocation_stall).collect();
    let degenerated: Vec<&GcEvent> = c.log.events.iter().filter(|e| e.flags.degenerated).collect();
    if stalls.is_empty() && degenerated.is_empty() {
        return None;
    }
    let minutes = (c.stats.span_ms / 60_000.0).max(1.0);
    let per_min = stalls.len() as f64 / minutes;
    let severity = if per_min >= c.th.stall_warning_per_min || !degenerated.is_empty() {
        "warning"
    } else if per_min >= c.th.stall_info_per_min {
        "info"
    } else {
        return None;
    };
    Some(finding(
        "GC14",
        severity,
        format!("stalls={};stalls_per_min={};degenerated={}", stalls.len(), round(per_min, 2), degenerated.len()),
        anchors(stalls.into_iter().chain(degenerated)),
    ))
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "critical" => 0,
        "warning" => 1,
        _ => 2,
    }
}

/// Evaluate all rules for one JVM group with explicit thresholds.
pub fn evaluate_with(log: &ParsedGcLog, stats: &GcStats, th: &GcThresholds) -> Vec<GcFinding> {
    let c = Ctx {
        log,
        stats,
        th,
        pauses: log.events.iter().filter(|e| e.is_pause).collect(),
        memory: log.events.iter().filter(|e| e.heap.is_some() || e.old.is_some()).collect(),
    };
    let rules: [fn(&Ctx) -> Option<GcFinding>; 14] = [
        full_gc,
        consecutive_full,
        system_gc,
        evacuation_failure,
        cms_failure,
        humongous,
        long_pauses,
        pause_ratio,
        leak_trend,
        heap_after_high,
        metaspace,
        parallelism,
        frequent_young,
        concurrent_degradation,
    ];
    let mut out: Vec<GcFinding> = rules.iter().filter_map(|r| r(&c)).collect();
    small_sort_by(&mut out, |a, b| severity_rank(&a.severity).cmp(&severity_rank(&b.severity)).then(a.rule.cmp(&b.rule)));
    out
}

/// Evaluate all rules for one JVM group with the default thresholds.
pub fn evaluate(log: &ParsedGcLog, stats: &GcStats) -> Vec<GcFinding> {
    evaluate_with(log, stats, &GcThresholds::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::model::{CpuTimes, EventFlags, GcLogFormat, JvmInfo};
    use crate::gc::{parse_log, stats::compute};

    fn ev(t: f64, dur: f64, kind: GcKind, cause: &str) -> GcEvent {
        GcEvent {
            gc_id: None,
            time_ms: t,
            uptime_ms: Some(t),
            wall_ms: None,
            kind,
            is_pause: kind != GcKind::Concurrent,
            name: "Pause".into(),
            cause: Some(cause.into()),
            duration_ms: dur,
            heap: None,
            young: None,
            old: None,
            humongous: None,
            metaspace: None,
            cpu: None,
            flags: EventFlags::default(),
            file_index: 0,
            line: 0,
        }
    }

    fn mem(before: f64, after: f64, cap: f64) -> Option<MemChange> {
        Some(MemChange { before_kb: before, after_kb: after, capacity_before_kb: Some(cap), capacity_after_kb: Some(cap) })
    }

    fn log_of(collector: Collector, events: Vec<GcEvent>, heap_max_kb: Option<f64>) -> ParsedGcLog {
        ParsedGcLog {
            format: GcLogFormat::Unified,
            collector,
            jvm: JvmInfo { heap_max_kb, ..Default::default() },
            events,
            line_count: 0,
            unparsed_count: 0,
            unparsed_samples: Vec::new(),
            pid: None,
            missing_timestamps: false,
            first_line_uptime_ms: None,
            last_line_uptime_ms: None,
            first_line_wall_ms: None,
            last_line_wall_ms: None,
            rotation_created: None,
            rotation_saved_as: None,
        }
    }

    fn run(log: &ParsedGcLog) -> Vec<GcFinding> {
        let stats = compute(&log.events, None, log.jvm.heap_max_kb);
        evaluate(log, &stats)
    }

    fn rule<'a>(f: &'a [GcFinding], id: &str) -> Option<&'a GcFinding> {
        f.iter().find(|x| x.rule == id)
    }

    /// Healthy young-only series: 1 s apart, 5 ms each, flat heap.
    fn healthy(n: usize) -> Vec<GcEvent> {
        (0..n)
            .map(|i| {
                let mut e = ev(1000.0 * (i as f64 + 1.0), 5.0, GcKind::Young, "G1 Evacuation Pause");
                e.heap = mem(500.0 * 1024.0, 100.0 * 1024.0, 1024.0 * 1024.0);
                e.cpu = Some(CpuTimes { user_ms: 20.0, sys_ms: 1.0, real_ms: 5.0 });
                e
            })
            .collect()
    }

    #[test]
    fn findings_clean_log_has_none() {
        let f = run(&log_of(Collector::G1, healthy(40), Some(1024.0 * 1024.0)));
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn findings_gc01_full_gc_only_on_g1_cms() {
        let mut events = healthy(20);
        events.push(ev(30_000.0, 80.0, GcKind::Full, "G1 Compaction Pause"));
        events.push(ev(31_000.0, 20.0, GcKind::Full, "System.gc()"));
        events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        let f = run(&log_of(Collector::G1, events.clone(), None));
        let g = rule(&f, "GC01").unwrap();
        assert_eq!(g.severity, "warning");
        assert!(g.detail.starts_with("count=1;") && g.detail.contains("G1 Compaction Pause:1"), "{}", g.detail);
        assert_eq!(g.anchors, vec![(30_000.0, 30_080.0)]);
        // Parallel Full GCs are normal old-gen collections: not applicable.
        assert!(rule(&run(&log_of(Collector::Parallel, events, None)), "GC01").is_none());
    }

    #[test]
    fn findings_gc02_consecutive_full_warning_and_critical() {
        let mut events = healthy(5);
        for i in 0..4 {
            let mut e = ev(10_000.0 + 2000.0 * i as f64, 900.0, GcKind::Full, "Ergonomics");
            // Old gen nearly full after each Full GC: 0.007% free (like a real Parallel OOM spiral).
            e.old = mem(5_592_222.0, 5_592_201.0, 5_592_576.0);
            e.heap = mem(6_524_569.0, 6_054_830.0, 7_456_768.0);
            events.push(e);
        }
        let f = run(&log_of(Collector::Parallel, events.clone(), None));
        let g = rule(&f, "GC02").unwrap();
        assert_eq!(g.severity, "critical");
        assert!(g.detail.contains("runs=1;longest=4;full=4;starved_runs=1"), "{}", g.detail);
        assert_eq!(g.anchors, vec![(10_000.0, 16_900.0)]);
        // Same run but the old gen keeps plenty free and each frees a lot → warning only.
        for e in events.iter_mut().filter(|e| e.kind == GcKind::Full) {
            e.old = mem(4_000_000.0, 1_000_000.0, 5_592_576.0);
        }
        assert_eq!(rule(&run(&log_of(Collector::Parallel, events.clone(), None)), "GC02").unwrap().severity, "warning");
        // A young GC between them breaks the run.
        let mut broken = events.clone();
        broken.push(ev(13_000.0, 5.0, GcKind::Young, "Allocation Failure"));
        broken.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        assert!(rule(&run(&log_of(Collector::Parallel, broken, None)), "GC02").is_none());
    }

    #[test]
    fn findings_gc03_system_gc_ratio_and_hourly() {
        let mut events = healthy(10);
        for i in 0..3 {
            events.push(ev(20_000.0 + 3_600_000.0 * i as f64, 30.0, GcKind::Full, "System.gc()"));
        }
        events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        let g = rule(&run(&log_of(Collector::G1, events, None)), "GC03").unwrap().clone();
        assert_eq!(g.severity, "info");
        assert!(g.detail.contains("count=3") && g.detail.contains("hourly=1"), "{}", g.detail);
        let mut many: Vec<GcEvent> = (0..6).map(|i| ev(1000.0 * i as f64, 10.0, GcKind::Full, "System.gc()")).collect();
        many.extend(healthy(2));
        many.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        assert_eq!(rule(&run(&log_of(Collector::G1, many, None)), "GC03").unwrap().severity, "warning");
    }

    #[test]
    fn findings_gc04_gc05_failures_are_critical() {
        let mut events = healthy(5);
        events[2].flags.evacuation_failure = true;
        events[3].flags.promotion_failed = true;
        events[4].flags.concurrent_mode_failure = true;
        let f = run(&log_of(Collector::G1, events, None));
        assert_eq!(rule(&f, "GC04").unwrap().severity, "critical");
        let g5 = rule(&f, "GC05").unwrap();
        assert_eq!(g5.detail, "promotion_failed=1;concurrent_mode_failure=1");
        assert_eq!(f[0].severity, "critical", "critical findings sort first");
    }

    #[test]
    fn findings_gc07_long_pause_levels() {
        let mut events = healthy(200);
        events[100].duration_ms = 350.0; // one pause over the 200 ms G1 target, P99 still fine
        let f = run(&log_of(Collector::G1, events.clone(), None));
        assert_eq!(rule(&f, "GC07").unwrap().severity, "info");
        events[101].duration_ms = 1500.0; // over 1 s
        let g = run(&log_of(Collector::G1, events, None));
        let g7 = rule(&g, "GC07").unwrap();
        assert_eq!(g7.severity, "warning");
        assert!(g7.detail.contains("target_ms=200") && g7.detail.contains("count_over=2"), "{}", g7.detail);
        assert_eq!(g7.anchors[0], (102_000.0, 103_500.0), "longest first");
    }

    #[test]
    fn findings_gc08_window_ratio() {
        // 60 pauses of 120 ms within one minute = 12% in the worst window → warning.
        let mut events = healthy(120);
        for e in events.iter_mut().skip(30).take(60) {
            e.duration_ms = 120.0;
        }
        let g = rule(&run(&log_of(Collector::G1, events, None)), "GC08").unwrap().clone();
        assert_eq!(g.severity, "warning", "{}", g.detail);
        assert!(g.detail.starts_with("window_pct=12"), "{}", g.detail);
        // Parallel over its 1% goal globally but with a quiet worst window → info.
        let mut par = healthy(120);
        for e in par.iter_mut() {
            e.duration_ms = 20.0; // 2% everywhere
        }
        assert_eq!(rule(&run(&log_of(Collector::Parallel, par, None)), "GC08").unwrap().severity, "info");
    }

    #[test]
    fn findings_gc09_leak_trend() {
        // One young GC per minute for 2 h; old gen after GC grows 10 MB/min with tiny noise; one Full GC exists.
        let max = 8.0 * 1024.0 * 1024.0;
        let mut events: Vec<GcEvent> = (0..120)
            .map(|i| {
                let t = 60_000.0 * (i as f64 + 1.0);
                let mut e = ev(t, 10.0, GcKind::Young, "G1 Evacuation Pause");
                let old = 1024.0 * 1024.0 + 10.0 * 1024.0 * i as f64 + if i % 2 == 0 { 500.0 } else { -500.0 };
                e.old = Some(MemChange { before_kb: old - 1000.0, after_kb: old, capacity_before_kb: None, capacity_after_kb: None });
                e.heap = mem(old + 2_000_000.0, old + 50_000.0, max);
                e
            })
            .collect();
        events.push(ev(7_300_000.0, 500.0, GcKind::Full, "G1 Compaction Pause"));
        let f = run(&log_of(Collector::G1, events.clone(), Some(max)));
        let g = rule(&f, "GC09").unwrap();
        // 10 MB/min = 600 MB/h; ~6 GB left at 600 MB/h → ~10 h to full (< 24 h) → warning.
        assert_eq!(g.severity, "warning", "{}", g.detail);
        let slope: f64 = g.detail.split(';').next().unwrap().trim_start_matches("slope_mb_per_h=").parse().unwrap();
        assert!((slope - 600.0).abs() < 1.0, "{}", g.detail);
        assert!(g.detail.contains("hours_to_full=10;"), "{}", g.detail);
        // Without any old-gen collection, at most info (JMC caps it below WARNING).
        events.pop();
        assert_eq!(rule(&run(&log_of(Collector::G1, events.clone(), Some(max))), "GC09").unwrap().severity, "info");
        // Flat old gen: no finding. Short log (< 30 min after warm-up): no finding.
        for (i, e) in events.iter_mut().enumerate() {
            e.old = Some(MemChange { before_kb: 1e6, after_kb: 1e6 + if i % 2 == 0 { 9e4 } else { -9e4 }, capacity_before_kb: None, capacity_after_kb: None });
        }
        assert!(rule(&run(&log_of(Collector::G1, events, Some(max))), "GC09").is_none());
    }

    #[test]
    fn findings_gc10_heap_after_levels() {
        let mut events = healthy(10);
        events[5].heap = mem(1000.0 * 1024.0, 900.0 * 1024.0, 1024.0 * 1024.0); // 87.9%
        let f = run(&log_of(Collector::G1, events.clone(), Some(1024.0 * 1024.0)));
        assert_eq!(rule(&f, "GC10").unwrap().severity, "warning");
        events[6].heap = mem(1024.0 * 1024.0, 1010.0 * 1024.0, 1024.0 * 1024.0); // 98.6%
        let g = run(&log_of(Collector::G1, events, Some(1024.0 * 1024.0)));
        let g10 = rule(&g, "GC10").unwrap();
        assert_eq!(g10.severity, "critical");
        assert!(g10.detail.contains("count_over=2"), "{}", g10.detail);
    }

    #[test]
    fn findings_gc11_gc12_gc13_gc14() {
        // GC11: metaspace grows 1 MiB/s after warm-up.
        let mut events: Vec<GcEvent> = (0..30)
            .map(|i| {
                let t = 400_000.0 + 1000.0 * i as f64;
                let mut e = ev(t, 5.0, GcKind::Young, if i < 3 { "Metadata GC Threshold" } else { "G1 Evacuation Pause" });
                e.metaspace = Some(MemChange { before_kb: 0.0, after_kb: 50_000.0 + 1024.0 * i as f64, capacity_before_kb: None, capacity_after_kb: None });
                e
            })
            .collect();
        // GC12: one 300 ms pause with almost no CPU → inverted parallelism.
        events[10].duration_ms = 300.0;
        events[10].cpu = Some(CpuTimes { user_ms: 20.0, sys_ms: 10.0, real_ms: 300.0 });
        // GC13: young GCs every 1 s on average → not < 1 s; make them 500 ms apart instead.
        for (i, e) in events.iter_mut().enumerate() {
            e.time_ms = 400_000.0 + 500.0 * i as f64;
            e.uptime_ms = Some(e.time_ms);
        }
        // GC14: degenerated Shenandoah cycle.
        events[20].flags.degenerated = true;
        let f = run(&log_of(Collector::G1, events, None));
        let g11 = rule(&f, "GC11").unwrap();
        assert_eq!(g11.severity, "warning", "{}", g11.detail);
        assert!(g11.detail.ends_with("metadata_threshold=3"));
        assert_eq!(rule(&f, "GC12").unwrap().severity, "warning");
        assert_eq!(rule(&f, "GC13").unwrap().severity, "info");
        assert_eq!(rule(&f, "GC14").unwrap().severity, "warning");
    }

    #[test]
    fn findings_linear_fit_r2() {
        let (a, b, r2) = linear_fit(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)]).unwrap();
        assert_eq!((a, b, r2), (1.0, 2.0, 1.0));
        assert!(linear_fit(&[(1.0, 1.0)]).is_none());
        assert!(linear_fit(&[(1.0, 1.0), (1.0, 2.0)]).is_none());
    }

    #[test]
    fn findings_real_g1_stress_fixture() {
        let log = parse_log(include_str!("../../tests/fixtures/gc/g1-stress-jdk21.log"));
        let f = run(&log);
        let ids: Vec<&str> = f.iter().map(|x| x.rule.as_str()).collect();
        // Real JDK 21 G1 stress run: evacuation failures, G1 Compaction Full GCs, System.gc(), humongous, ~91% heap after GC.
        assert_eq!(rule(&f, "GC04").unwrap().detail, "count=9");
        assert_eq!(rule(&f, "GC04").unwrap().severity, "critical");
        assert!(rule(&f, "GC01").unwrap().detail.contains("G1 Compaction Pause:"), "{ids:?}");
        assert!(rule(&f, "GC03").is_some() && rule(&f, "GC06").is_some() && rule(&f, "GC10").is_some(), "{ids:?}");
        assert_eq!(f[0].severity, "critical");
        // The calm default-run fixture has no warnings or criticals.
        let calm = run(&parse_log(include_str!("../../tests/fixtures/gc/g1-default-jdk21.log")));
        assert!(calm.iter().all(|x| x.severity == "info"), "{calm:?}");
    }
}
