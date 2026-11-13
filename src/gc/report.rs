//! Whole-import GC report, chart series and event paging (feat-074).
//!
//! Shared by the WASM `GcSession` and the CLI so both see the same numbers.

use serde::{Deserialize, Serialize};

use super::findings::{evaluate, GcFinding};
use super::model::{Collector, GcEvent, GcKind, GcLogFormat, JvmInfo};
use super::stats::{allocation_intervals, compute, merged_rates, promotion_intervals, GcStats};
use super::stitch::{FileFingerprint, GcGroup, GroupNote, GroupingMode, Stitched};

/// Per-JVM summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupReport {
    pub index: usize,
    pub name: String,
    pub files: Vec<usize>,
    pub format: GcLogFormat,
    pub collector: Collector,
    pub jvm: JvmInfo,
    pub event_count: usize,
    pub missing_timestamps: bool,
    /// Every event has an uptime (the time axis is uptime).
    pub has_uptime: bool,
    /// Every event has a wall-clock time (a wall-clock axis is available).
    pub has_wall: bool,
    /// `wall_ms − time_ms` of the first event: converts the time axis to epoch ms.
    pub wall_offset_ms: Option<f64>,
    pub stats: GcStats,
    pub findings: Vec<GcFinding>,
    pub notes: Vec<GroupNote>,
    pub unparsed_count: usize,
    pub unparsed_samples: Vec<(usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcReport {
    pub mode: GroupingMode,
    pub files: Vec<FileFingerprint>,
    pub groups: Vec<GroupReport>,
}

pub fn group_report(index: usize, g: &GcGroup) -> GroupReport {
    let log = &g.log;
    let stats = compute(&log.events, None, log.jvm.heap_max_kb);
    let findings = evaluate(log, &stats);
    let has_uptime = !log.events.is_empty() && log.events.iter().all(|e| e.uptime_ms.is_some());
    let has_wall = !log.events.is_empty() && log.events.iter().all(|e| e.wall_ms.is_some());
    let wall_offset_ms = log.events.iter().find_map(|e| e.wall_ms.map(|w| w - e.time_ms));
    GroupReport {
        index,
        name: g.name.clone(),
        files: g.files.clone(),
        format: log.format,
        collector: log.collector,
        jvm: log.jvm.clone(),
        event_count: log.events.len(),
        missing_timestamps: log.missing_timestamps,
        has_uptime,
        has_wall,
        wall_offset_ms,
        stats,
        findings,
        notes: g.notes.clone(),
        unparsed_count: log.unparsed_count,
        unparsed_samples: log.unparsed_samples.clone(),
    }
}

pub fn build_report(stitched: &Stitched) -> GcReport {
    GcReport {
        mode: stitched.mode,
        files: stitched.files.clone(),
        groups: stitched.groups.iter().enumerate().map(|(i, g)| group_report(i, g)).collect(),
    }
}

/// Names of the per-event chart columns, in [`series_column`] order.
pub const SERIES_COLUMNS: [&str; 21] = [
    "time_ms",
    "wall_ms",
    "duration_ms",
    "kind",
    "pause",
    "flags",
    "gc_id",
    "heap_before_kb",
    "heap_after_kb",
    "heap_capacity_kb",
    "young_before_kb",
    "young_after_kb",
    "old_before_kb",
    "old_after_kb",
    "humongous_after_kb",
    "metaspace_after_kb",
    "metaspace_capacity_kb",
    "cpu_user_ms",
    "cpu_sys_ms",
    "cpu_real_ms",
    "alloc_rate_mb_s",
];

/// Extra derived column names (also available through [`series_column`]).
pub const DERIVED_COLUMNS: [&str; 1] = ["promotion_rate_mb_s"];

/// Flag bits of the `flags` column.
pub const FLAG_EVACUATION_FAILURE: u32 = 1;
pub const FLAG_PROMOTION_FAILED: u32 = 2;
pub const FLAG_CONCURRENT_MODE_FAILURE: u32 = 4;
pub const FLAG_ALLOCATION_STALL: u32 = 8;
pub const FLAG_DEGENERATED: u32 = 16;

fn flag_bits(e: &GcEvent) -> u32 {
    let f = e.flags;
    (f.evacuation_failure as u32) * FLAG_EVACUATION_FAILURE
        | (f.promotion_failed as u32) * FLAG_PROMOTION_FAILED
        | (f.concurrent_mode_failure as u32) * FLAG_CONCURRENT_MODE_FAILURE
        | (f.allocation_stall as u32) * FLAG_ALLOCATION_STALL
        | (f.degenerated as u32) * FLAG_DEGENERATED
}

/// Per-event rate column from `(index, amount, interval)` triples (NaN elsewhere).
fn rate_column(len: usize, intervals: &[(usize, f64, f64)]) -> Vec<f64> {
    let mut out = vec![f64::NAN; len];
    for (i, rate) in merged_rates(intervals) {
        out[i] = rate;
    }
    out
}

/// One chart column for `events` (NaN marks a missing value). `None` for unknown names.
pub fn series_column(events: &[GcEvent], name: &str) -> Option<Vec<f64>> {
    let nan = f64::NAN;
    let col = |f: &dyn Fn(&GcEvent) -> Option<f64>| events.iter().map(|e| f(e).unwrap_or(nan)).collect();
    Some(match name {
        "time_ms" => col(&|e| Some(e.time_ms)),
        "wall_ms" => col(&|e| e.wall_ms),
        "duration_ms" => col(&|e| Some(e.duration_ms)),
        "kind" => col(&|e| Some(e.kind.code() as f64)),
        "pause" => col(&|e| Some(if e.is_pause { 1.0 } else { 0.0 })),
        "flags" => col(&|e| Some(flag_bits(e) as f64)),
        "gc_id" => col(&|e| e.gc_id.map(|v| v as f64)),
        "heap_before_kb" => col(&|e| e.heap.map(|m| m.before_kb)),
        "heap_after_kb" => col(&|e| e.heap.map(|m| m.after_kb)),
        "heap_capacity_kb" => col(&|e| e.heap.and_then(|m| m.capacity_after_kb)),
        "young_before_kb" => col(&|e| e.young.map(|m| m.before_kb)),
        "young_after_kb" => col(&|e| e.young.map(|m| m.after_kb)),
        "old_before_kb" => col(&|e| e.old.map(|m| m.before_kb)),
        "old_after_kb" => col(&|e| e.old.map(|m| m.after_kb)),
        "humongous_after_kb" => col(&|e| e.humongous.map(|m| m.after_kb)),
        "metaspace_after_kb" => col(&|e| e.metaspace.map(|m| m.after_kb)),
        "metaspace_capacity_kb" => col(&|e| e.metaspace.and_then(|m| m.capacity_after_kb)),
        "cpu_user_ms" => col(&|e| e.cpu.map(|c| c.user_ms)),
        "cpu_sys_ms" => col(&|e| e.cpu.map(|c| c.sys_ms)),
        "cpu_real_ms" => col(&|e| e.cpu.map(|c| c.real_ms)),
        "alloc_rate_mb_s" => rate_column(events.len(), &allocation_intervals(&events.iter().collect::<Vec<_>>())),
        "promotion_rate_mb_s" => rate_column(events.len(), &promotion_intervals(&events.iter().collect::<Vec<_>>())),
        _ => return None,
    })
}

/// Event table sort column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSort {
    #[default]
    Time,
    Duration,
}

/// Event table query (all fields optional from JS).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EventQuery {
    pub offset: usize,
    pub limit: Option<usize>,
    pub range: Option<(f64, f64)>,
    pub kinds: Option<Vec<GcKind>>,
    /// Exact cause match (the cause column falls back to the event name).
    pub cause: Option<String>,
    pub min_duration_ms: Option<f64>,
    pub pauses_only: bool,
    pub sort: EventSort,
    pub desc: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventRow {
    /// Index into the group's time-ordered events (= series row).
    pub index: usize,
    #[serde(flatten)]
    pub event: GcEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventPage {
    /// Matching events before paging.
    pub total: usize,
    pub offset: usize,
    pub rows: Vec<EventRow>,
}

/// Default and maximum page size.
pub const DEFAULT_PAGE: usize = 200;
pub const MAX_PAGE: usize = 5000;

pub fn query_events(events: &[GcEvent], q: &EventQuery) -> EventPage {
    let mut idx: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            q.range.is_none_or(|(t0, t1)| e.time_ms >= t0 && e.time_ms <= t1)
                && q.kinds.as_ref().is_none_or(|ks| ks.contains(&e.kind))
                && q.cause.as_ref().is_none_or(|c| e.cause.as_ref().unwrap_or(&e.name) == c)
                && q.min_duration_ms.is_none_or(|m| e.duration_ms >= m)
                && (!q.pauses_only || e.is_pause)
        })
        .map(|(i, _)| i)
        .collect();
    if q.sort == EventSort::Duration {
        idx.sort_unstable_by(|&a, &b| events[a].duration_ms.total_cmp(&events[b].duration_ms).then(a.cmp(&b)));
    }
    if q.desc {
        idx.reverse();
    }
    let total = idx.len();
    let limit = q.limit.unwrap_or(DEFAULT_PAGE).min(MAX_PAGE);
    let rows = idx
        .into_iter()
        .skip(q.offset)
        .take(limit)
        .map(|i| EventRow { index: i, event: events[i].clone() })
        .collect();
    EventPage { total, offset: q.offset, rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::stitch::{stitch, GcInput};

    const G1_STRESS: &str = include_str!("../../tests/fixtures/gc/g1-stress-jdk21.log");
    const PARALLEL: &str = include_str!("../../tests/fixtures/gc/parallel-jdk21.log");

    fn report() -> (Stitched, GcReport) {
        let s = stitch(
            &[GcInput { name: "g1.log", text: G1_STRESS }, GcInput { name: "par.log", text: PARALLEL }],
            GroupingMode::Auto,
        );
        let r = build_report(&s);
        (s, r)
    }

    #[test]
    fn report_groups_have_stats_and_axes() {
        let (_, r) = report();
        assert_eq!(r.groups.len(), 2);
        let g1 = r.groups.iter().find(|g| g.collector == Collector::G1).unwrap();
        assert_eq!(g1.event_count, 147);
        assert_eq!(g1.stats.pause_count, 107);
        assert!(g1.has_uptime && g1.has_wall);
        assert!(g1.wall_offset_ms.unwrap() > 1.7e12);
        assert_eq!(r.files.len(), 2);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"collector\":\"g1\"") && json.contains("\"mode\":\"auto\""));
    }

    #[test]
    fn report_series_columns() {
        let (s, _) = report();
        let ev = &s.groups.iter().find(|g| g.log.collector == Collector::G1).unwrap().log.events;
        for name in SERIES_COLUMNS.iter().chain(DERIVED_COLUMNS.iter()) {
            let col = series_column(ev, name).unwrap();
            assert_eq!(col.len(), ev.len(), "{name}");
        }
        assert!(series_column(ev, "nope").is_none());
        let kind = series_column(ev, "kind").unwrap();
        let full = GcKind::Full.code() as f64;
        assert_eq!(kind.iter().filter(|&&k| k == full).count(), 10);
        let flags = series_column(ev, "flags").unwrap();
        assert_eq!(flags.iter().filter(|&&f| (f as u32) & FLAG_EVACUATION_FAILURE != 0).count(), 9);
        let rate = series_column(ev, "alloc_rate_mb_s").unwrap();
        assert!(rate.iter().any(|r| r.is_finite() && *r > 0.0));
        // Concurrent events have no heap value.
        let heap = series_column(ev, "heap_after_kb").unwrap();
        let pause = series_column(ev, "pause").unwrap();
        assert!(heap.iter().zip(&pause).all(|(h, p)| *p == 1.0 || h.is_nan()));
    }

    #[test]
    fn report_event_query_filters_sorts_and_pages() {
        let (s, _) = report();
        let ev = &s.groups.iter().find(|g| g.log.collector == Collector::G1).unwrap().log.events;
        let all = query_events(ev, &EventQuery::default());
        assert_eq!((all.total, all.rows.len()), (147, DEFAULT_PAGE.min(147)));
        let full = query_events(ev, &EventQuery { kinds: Some(vec![GcKind::Full]), ..Default::default() });
        assert_eq!(full.total, 10);
        let longest = query_events(ev, &EventQuery { sort: EventSort::Duration, desc: true, limit: Some(3), ..Default::default() });
        assert_eq!(longest.rows.len(), 3);
        assert!(longest.rows[0].event.duration_ms >= longest.rows[1].event.duration_ms);
        let page2 = query_events(ev, &EventQuery { offset: 140, limit: Some(50), ..Default::default() });
        assert_eq!((page2.total, page2.rows.len(), page2.rows[0].index), (147, 7, 140));
        let sysgc = query_events(ev, &EventQuery { cause: Some("System.gc()".into()), ..Default::default() });
        assert!(sysgc.total > 0 && sysgc.rows.iter().all(|r| r.event.cause.as_deref() == Some("System.gc()")));
        let t_mid = ev[70].time_ms;
        let ranged = query_events(ev, &EventQuery { range: Some((0.0, t_mid)), ..Default::default() });
        assert!(ranged.total >= 71 && ranged.total < 147);
        // JSON shape from JS.
        let q: EventQuery =
            serde_json::from_str(r#"{"kinds":["full","young"],"sort":"duration","desc":true,"pauses_only":true}"#).unwrap();
        assert!(q.pauses_only && q.desc && q.sort == EventSort::Duration);
        assert_eq!(q.kinds.unwrap().len(), 2);
        let pauses = query_events(ev, &EventQuery { pauses_only: true, ..Default::default() });
        assert_eq!(pauses.total, 107);
    }
}
