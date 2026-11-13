//! GC statistics engine (feat-071): KPIs, percentiles, rates, causes and a
//! pause histogram over a whole log or an optional time range. The web range
//! recompute and the CLI share this single implementation.
//!
//! Conventions:
//! - An event belongs to a range when its start `time_ms` is inside `[t0, t1]`.
//! - The span is clipped to the data: `[max(t0, first start), min(t1, last end)]`.
//! - Percentiles use the nearest-rank method over pause durations.
//! - Allocation is measured between consecutive pauses that report memory:
//!   `young.before(n) − young.after(n−1)` (whole heap when generations are missing).

use std::collections::HashMap;

use serde::Serialize;

use super::model::{GcEvent, GcKind};
use super::units::small_sort_by;

/// Rate intervals shorter than this are merged into the next one (back-to-back pauses).
const MIN_PEAK_INTERVAL_MS: f64 = 50.0;

/// Pause histogram bucket edges in ms (1-2-5 series); the last bucket is open-ended.
const HISTOGRAM_EDGES_MS: [f64; 16] =
    [0.0, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Percentiles {
    pub mean: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KindStat {
    pub kind: GcKind,
    pub count: usize,
    pub total_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CauseStat {
    /// Cause text, or the event name when the log prints no cause.
    pub cause: String,
    pub count: usize,
    pub total_pause_ms: f64,
    pub max_pause_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RateStats {
    pub avg_mb_per_s: f64,
    pub peak_mb_per_s: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct HistogramBucket {
    pub lo_ms: f64,
    /// `None` for the open-ended last bucket.
    pub hi_ms: Option<f64>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcStats {
    /// Requested range, if any.
    pub range: Option<(f64, f64)>,
    /// Clipped start / end of the analyzed window in ms.
    pub start_ms: f64,
    pub end_ms: f64,
    pub span_ms: f64,
    pub event_count: usize,
    pub pause_count: usize,
    pub total_pause_ms: f64,
    /// `100 × (1 − total pause / span)`; `None` when the span is zero.
    pub throughput_pct: Option<f64>,
    pub pause: Option<Percentiles>,
    /// Mean gap between consecutive pause starts.
    pub mean_pause_interval_ms: Option<f64>,
    pub kinds: Vec<KindStat>,
    pub concurrent_ms: f64,
    pub concurrent_pct: Option<f64>,
    pub allocation: Option<RateStats>,
    pub promotion: Option<RateStats>,
    pub heap_after_p50_kb: Option<f64>,
    pub heap_after_max_kb: Option<f64>,
    /// Peak of `heap after / max heap` (or committed capacity when the max is unknown).
    pub heap_after_peak_pct: Option<f64>,
    pub metaspace_peak_kb: Option<f64>,
    /// Sorted by total pause time, descending.
    pub causes: Vec<CauseStat>,
    /// Buckets from the first to the last non-empty one.
    pub histogram: Vec<HistogramBucket>,
}

/// Nearest-rank percentile of an ascending slice.
fn nearest_rank(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let rank = ((p / 100.0) * n as f64).ceil().max(1.0) as usize;
    sorted[rank.min(n) - 1]
}

fn percentiles(values: &mut [f64]) -> Option<Percentiles> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable_by(f64::total_cmp);
    Some(Percentiles {
        mean: values.iter().sum::<f64>() / values.len() as f64,
        p50: nearest_rank(values, 50.0),
        p90: nearest_rank(values, 90.0),
        p95: nearest_rank(values, 95.0),
        p99: nearest_rank(values, 99.0),
        max: *values.last().unwrap(),
    })
}

pub fn kb_per_ms_to_mb_per_s(kb: f64, ms: f64) -> f64 {
    (kb / 1024.0) / (ms / 1000.0)
}

/// KiB allocated between pause `prev` and pause `cur` (young gen when both report it, else heap).
pub fn allocated_between(prev: &GcEvent, cur: &GcEvent) -> f64 {
    match (cur.young, prev.young) {
        (Some(c), Some(p)) => c.before_kb - p.after_kb,
        _ => match (cur.heap, prev.heap) {
            (Some(c), Some(p)) => c.before_kb - p.after_kb,
            _ => 0.0,
        },
    }
    .max(0.0)
}

/// Rate points `(event index, MB/s)` from per-interval amounts. Intervals shorter than
/// [`MIN_PEAK_INTERVAL_MS`] (back-to-back pauses) are merged into the next one so a few
/// KiB over a millisecond never reads as GB/s. Shared by the KPIs and the chart series.
pub fn merged_rates(intervals: &[(usize, f64, f64)]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let (mut kb, mut dt) = (0.0, 0.0);
    for &(idx, amount_kb, interval_ms) in intervals {
        kb += amount_kb;
        dt += interval_ms;
        if dt >= MIN_PEAK_INTERVAL_MS {
            out.push((idx, kb_per_ms_to_mb_per_s(kb, dt)));
            kb = 0.0;
            dt = 0.0;
        }
    }
    out
}

/// `(index, allocated KiB, interval ms)` between consecutive memory-reporting events
/// (pauses, or ZGC / Shenandoah cycles, whose heap figures sit on concurrent events).
pub fn allocation_intervals(events: &[&GcEvent]) -> Vec<(usize, f64, f64)> {
    let mut out = Vec::new();
    let mut prev: Option<usize> = None;
    for (i, e) in events.iter().enumerate() {
        if e.heap.is_none() && e.young.is_none() {
            continue;
        }
        if let Some(p) = prev {
            out.push((i, allocated_between(events[p], e), e.time_ms - events[p].time_ms));
        }
        prev = Some(i);
    }
    out
}

/// `(index, promoted KiB, interval ms)` between consecutive young pauses that report the old gen.
pub fn promotion_intervals(events: &[&GcEvent]) -> Vec<(usize, f64, f64)> {
    let mut out = Vec::new();
    let mut prev: Option<usize> = None;
    for (i, e) in events.iter().enumerate() {
        if e.kind != GcKind::Young {
            continue;
        }
        let Some(old) = e.old else { continue };
        if let Some(p) = prev {
            out.push((i, (old.after_kb - old.before_kb).max(0.0), e.time_ms - events[p].time_ms));
        }
        prev = Some(i);
    }
    out
}

fn rate_stats(intervals: &[(usize, f64, f64)]) -> Option<RateStats> {
    let span: f64 = intervals.iter().map(|i| i.2).sum();
    if span <= 0.0 {
        return None;
    }
    let total: f64 = intervals.iter().map(|i| i.1).sum();
    let peak = merged_rates(intervals).iter().map(|r| r.1).fold(0.0, f64::max);
    Some(RateStats { avg_mb_per_s: kb_per_ms_to_mb_per_s(total, span), peak_mb_per_s: peak })
}

fn histogram(durations: &[f64]) -> Vec<HistogramBucket> {
    let mut buckets: Vec<HistogramBucket> = HISTOGRAM_EDGES_MS
        .iter()
        .enumerate()
        .map(|(i, &lo)| HistogramBucket { lo_ms: lo, hi_ms: HISTOGRAM_EDGES_MS.get(i + 1).copied(), count: 0 })
        .collect();
    for &d in durations {
        let idx = HISTOGRAM_EDGES_MS.iter().rposition(|&lo| d >= lo).unwrap_or(0);
        buckets[idx].count += 1;
    }
    let first = buckets.iter().position(|b| b.count > 0);
    let last = buckets.iter().rposition(|b| b.count > 0);
    match (first, last) {
        (Some(a), Some(b)) => buckets[a..=b].to_vec(),
        _ => Vec::new(),
    }
}

/// Compute statistics over `events` (sorted by `time_ms`), optionally restricted to `range`.
///
/// `heap_max_kb` is the configured maximum heap, used for occupancy when known.
pub fn compute(events: &[GcEvent], range: Option<(f64, f64)>, heap_max_kb: Option<f64>) -> GcStats {
    let in_range: Vec<&GcEvent> = events
        .iter()
        .filter(|e| range.is_none_or(|(t0, t1)| e.time_ms >= t0 && e.time_ms <= t1))
        .collect();
    let data_start = events.iter().map(|e| e.time_ms).fold(f64::INFINITY, f64::min);
    let data_end = events.iter().map(|e| e.time_ms + e.duration_ms).fold(f64::NEG_INFINITY, f64::max);
    let (start_ms, end_ms) = match (range, events.is_empty()) {
        (_, true) => (0.0, 0.0),
        (None, false) => (data_start, data_end),
        (Some((t0, t1)), false) => (t0.max(data_start), t1.min(data_end)),
    };
    let span_ms = (end_ms - start_ms).max(0.0);

    let pauses: Vec<&GcEvent> = in_range.iter().copied().filter(|e| e.is_pause).collect();
    let mut durations: Vec<f64> = pauses.iter().map(|e| e.duration_ms).collect();
    let total_pause_ms: f64 = durations.iter().sum();
    let throughput_pct = (span_ms > 0.0).then(|| (100.0 * (1.0 - total_pause_ms / span_ms)).clamp(0.0, 100.0));
    let hist = histogram(&durations);
    let pause = percentiles(&mut durations);

    let mean_pause_interval_ms = (pauses.len() >= 2)
        .then(|| (pauses.last().unwrap().time_ms - pauses[0].time_ms) / (pauses.len() - 1) as f64);

    let mut kinds: HashMap<GcKind, KindStat> = HashMap::new();
    for e in &in_range {
        let k = kinds.entry(e.kind).or_insert(KindStat { kind: e.kind, count: 0, total_ms: 0.0, max_ms: 0.0 });
        k.count += 1;
        k.total_ms += e.duration_ms;
        k.max_ms = k.max_ms.max(e.duration_ms);
    }
    let mut kinds: Vec<KindStat> = kinds.into_values().collect();
    small_sort_by(&mut kinds, |a, b| a.kind.cmp(&b.kind));

    let concurrent_ms: f64 = in_range.iter().filter(|e| e.kind == GcKind::Concurrent).map(|e| e.duration_ms).sum();
    let concurrent_pct = (span_ms > 0.0).then(|| (100.0 * concurrent_ms / span_ms).min(100.0));

    let mut causes: HashMap<String, CauseStat> = HashMap::new();
    for e in &pauses {
        let key = e.cause.clone().unwrap_or_else(|| e.name.clone());
        let c = causes.entry(key.clone()).or_insert(CauseStat { cause: key, count: 0, total_pause_ms: 0.0, max_pause_ms: 0.0 });
        c.count += 1;
        c.total_pause_ms += e.duration_ms;
        c.max_pause_ms = c.max_pause_ms.max(e.duration_ms);
    }
    let mut causes: Vec<CauseStat> = causes.into_values().collect();
    small_sort_by(&mut causes, |a, b| {
        b.total_pause_ms.total_cmp(&a.total_pause_ms).then(b.count.cmp(&a.count)).then(a.cause.cmp(&b.cause))
    });

    // Memory figures come from every event that reports the heap (ZGC / Shenandoah report it on cycles).
    let memory: Vec<&GcEvent> = in_range.iter().copied().filter(|e| e.heap.is_some()).collect();
    let mut heap_after: Vec<f64> = memory.iter().filter_map(|e| e.heap.map(|h| h.after_kb)).collect();
    heap_after.sort_unstable_by(f64::total_cmp);
    let heap_after_peak_pct = memory
        .iter()
        .filter_map(|e| {
            let h = e.heap?;
            let denom = heap_max_kb.or(h.capacity_after_kb)?;
            (denom > 0.0).then(|| 100.0 * h.after_kb / denom)
        })
        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))));
    let metaspace_peak_kb = in_range
        .iter()
        .filter_map(|e| e.metaspace.map(|m| m.after_kb))
        .fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))));

    GcStats {
        range,
        start_ms,
        end_ms,
        span_ms,
        event_count: in_range.len(),
        pause_count: pauses.len(),
        total_pause_ms,
        throughput_pct,
        pause,
        mean_pause_interval_ms,
        kinds,
        concurrent_ms,
        concurrent_pct,
        allocation: rate_stats(&allocation_intervals(&in_range)),
        promotion: rate_stats(&promotion_intervals(&pauses)),
        heap_after_p50_kb: (!heap_after.is_empty()).then(|| nearest_rank(&heap_after, 50.0)),
        heap_after_max_kb: heap_after.last().copied(),
        heap_after_peak_pct,
        metaspace_peak_kb,
        causes,
        histogram: hist,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::model::{EventFlags, MemChange};

    fn mem(before: f64, after: f64, cap: Option<f64>) -> Option<MemChange> {
        Some(MemChange { before_kb: before, after_kb: after, capacity_before_kb: None, capacity_after_kb: cap })
    }

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

    /// Four pauses over 10 s plus one concurrent cycle; numbers chosen by hand.
    fn sample() -> Vec<GcEvent> {
        let mut a = ev(0.0, 10.0, GcKind::Young, "Allocation Failure");
        a.heap = mem(1024.0, 200.0, Some(4096.0));
        a.young = mem(900.0, 100.0, None);
        a.old = mem(100.0, 100.0, None);
        let mut b = ev(1000.0, 20.0, GcKind::Young, "Allocation Failure");
        b.heap = mem(1124.0, 300.0, Some(4096.0)); // allocated 924 KiB (heap view)
        b.young = mem(1000.0, 100.0, None); // allocated 900 KiB since a.young.after=100
        b.old = mem(100.0, 200.0, None); // promoted 100 KiB
        b.metaspace = mem(500.0, 500.0, None);
        let mut c = ev(4000.0, 30.0, GcKind::Full, "Ergonomics");
        c.heap = mem(2000.0, 1000.0, Some(4096.0));
        c.metaspace = mem(600.0, 550.0, None);
        let d = ev(9000.0, 940.0, GcKind::Young, "Allocation Failure"); // no memory info
        let conc = ev(5000.0, 2000.0, GcKind::Concurrent, "x");
        let mut v = vec![a, b, c, conc, d];
        v.sort_by(|x, y| x.time_ms.total_cmp(&y.time_ms));
        v
    }

    #[test]
    fn stats_whole_log_kpis() {
        let s = compute(&sample(), None, Some(4096.0));
        // Span: first start 0 → last end 9000 + 940 = 9940 ms.
        assert_eq!((s.start_ms, s.end_ms, s.span_ms), (0.0, 9940.0, 9940.0));
        assert_eq!(s.event_count, 5);
        assert_eq!(s.pause_count, 4);
        assert_eq!(s.total_pause_ms, 1000.0);
        let tp = s.throughput_pct.unwrap();
        assert!((tp - 100.0 * (1.0 - 1000.0 / 9940.0)).abs() < 1e-9);
        let p = s.pause.unwrap();
        // Sorted durations [10, 20, 30, 940]: nearest-rank p50 = 2nd, p90/p95/p99 = 4th.
        assert_eq!((p.p50, p.p90, p.p99, p.max), (20.0, 940.0, 940.0, 940.0));
        assert_eq!(p.mean, 250.0);
        assert_eq!(s.mean_pause_interval_ms, Some(3000.0));
        assert_eq!(s.concurrent_ms, 2000.0);
        assert!((s.concurrent_pct.unwrap() - 100.0 * 2000.0 / 9940.0).abs() < 1e-9);
        let full = s.kinds.iter().find(|k| k.kind == GcKind::Full).unwrap();
        assert_eq!((full.count, full.total_ms), (1, 30.0));
        // Causes by total pause: Allocation Failure 970 ms (3) then Ergonomics 30 ms.
        assert_eq!(s.causes[0].cause, "Allocation Failure");
        assert_eq!((s.causes[0].count, s.causes[0].total_pause_ms, s.causes[0].max_pause_ms), (3, 970.0, 940.0));
        assert_eq!(s.causes[1].cause, "Ergonomics");
        // Heap after (3 pauses with heap): [200, 300, 1000].
        assert_eq!((s.heap_after_p50_kb, s.heap_after_max_kb), (Some(300.0), Some(1000.0)));
        assert!((s.heap_after_peak_pct.unwrap() - 100.0 * 1000.0 / 4096.0).abs() < 1e-9);
        assert_eq!(s.metaspace_peak_kb, Some(550.0));
    }

    #[test]
    fn stats_rates_hand_computed() {
        let s = compute(&sample(), None, None);
        // a→b: young 1000 − 100 = 900 KiB in 1 s; b→c: c has no young → heap 2000 − 300 = 1700 KiB in 3 s.
        // d has no memory info and is skipped. Total 2600 KiB over 4 s.
        let a = s.allocation.unwrap();
        assert!((a.avg_mb_per_s - (2600.0 / 1024.0) / 4.0).abs() < 1e-9);
        assert!((a.peak_mb_per_s - (900.0 / 1024.0) / 1.0).abs() < 1e-9);
        // Promotion between young a and b: 100 KiB in 1 s.
        let p = s.promotion.unwrap();
        assert!((p.avg_mb_per_s - 100.0 / 1024.0).abs() < 1e-9);
        // Without a max heap the committed capacity is the occupancy denominator.
        assert!((s.heap_after_peak_pct.unwrap() - 100.0 * 1000.0 / 4096.0).abs() < 1e-9);
    }

    #[test]
    fn stats_range_is_clipped_and_filtered() {
        let s = compute(&sample(), Some((500.0, 4500.0)), None);
        assert_eq!(s.pause_count, 2); // b, c
        assert_eq!((s.start_ms, s.end_ms), (500.0, 4500.0));
        assert_eq!(s.total_pause_ms, 50.0);
        assert!((s.throughput_pct.unwrap() - 100.0 * (1.0 - 50.0 / 4000.0)).abs() < 1e-9);
        // Range larger than the data is clipped to the data.
        let s = compute(&sample(), Some((-1e9, 1e9)), None);
        assert_eq!((s.start_ms, s.end_ms), (0.0, 9940.0));
        // Empty range.
        let s = compute(&sample(), Some((100.0, 200.0)), None);
        assert_eq!(s.pause_count, 0);
        assert!(s.pause.is_none() && s.histogram.is_empty() && s.allocation.is_none());
    }

    #[test]
    fn stats_histogram_buckets() {
        let s = compute(&sample(), None, None);
        // 10 → [10,20), 20 → [20,50), 30 → [20,50), 940 → [500,1000).
        let counts: Vec<(f64, usize)> = s.histogram.iter().map(|b| (b.lo_ms, b.count)).collect();
        assert_eq!(counts, vec![(10.0, 1), (20.0, 2), (50.0, 0), (100.0, 0), (200.0, 0), (500.0, 1)]);
        assert_eq!(s.histogram.last().unwrap().hi_ms, Some(1000.0));
        let open = histogram(&[12_000.0]);
        assert_eq!((open[0].lo_ms, open[0].hi_ms), (5000.0, None));
    }

    #[test]
    fn stats_short_intervals_are_merged_into_rates() {
        // Two back-to-back pauses 10 ms apart, then a normal 980 ms interval:
        // one rate point at index 3 over 1000 ms, never a GB/s spike at 1 or 2.
        let rates = merged_rates(&[(1, 100.0, 10.0), (2, 100.0, 10.0), (3, 1024.0, 980.0)]);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].0, 3);
        assert!((rates[0].1 - (1224.0 / 1024.0)).abs() < 1e-9);
        assert!(merged_rates(&[(1, 5.0, 3.0)]).is_empty());
    }

    #[test]
    fn stats_empty_input() {
        let s = compute(&[], None, None);
        assert_eq!((s.span_ms, s.pause_count), (0.0, 0));
        assert!(s.throughput_pct.is_none() && s.pause.is_none());
    }

    #[test]
    fn stats_real_g1_stress_fixture_is_consistent() {
        let log = crate::gc::parse_log(include_str!("../../tests/fixtures/gc/g1-stress-jdk21.log"));
        let s = compute(&log.events, None, log.jvm.heap_max_kb);
        assert_eq!(s.pause_count, 107);
        let full = s.kinds.iter().find(|k| k.kind == GcKind::Full).unwrap();
        assert_eq!(full.count, 10);
        let p = s.pause.unwrap();
        assert!(p.p50 <= p.p90 && p.p90 <= p.p99 && p.p99 <= p.max);
        let tp = s.throughput_pct.unwrap();
        assert!(tp > 0.0 && tp < 100.0);
        assert!(s.allocation.unwrap().avg_mb_per_s > 0.0);
        assert!(s.heap_after_peak_pct.unwrap() <= 100.0);
        assert_eq!(s.histogram.iter().map(|b| b.count).sum::<usize>(), 107);
        assert!(s.causes.iter().any(|c| c.cause == "System.gc()"));
    }
}
