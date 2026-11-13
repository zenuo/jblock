//! GC log analysis for the CLI (feat-080): text report and exit status.

use crate::gc::findings::GcFinding;
use crate::gc::report::GcReport;
use crate::gc::stitch::Stitched;
use crate::gc::{Collector, GcEvent, GcKind};

use super::{CliExit, CliOptions, Section, SeverityFilter, Style};

/// Short English title per rule (the web UI carries the full localized text).
pub fn rule_title(rule: &str) -> &'static str {
    match rule {
        "GC01" => "Full GC occurred",
        "GC02" => "Back-to-back Full GCs",
        "GC03" => "Explicit System.gc() calls",
        "GC04" => "Evacuation failure (to-space exhausted)",
        "GC05" => "CMS promotion / concurrent mode failure",
        "GC06" => "Frequent humongous allocations",
        "GC07" => "Long GC pauses",
        "GC08" => "High GC time",
        "GC09" => "Live set keeps growing (possible leak)",
        "GC10" => "Heap nearly full after GC",
        "GC11" => "Metaspace keeps growing",
        "GC12" => "GC threads waiting on the system",
        "GC13" => "Very frequent young GCs",
        "GC14" => "Concurrent collector falling behind",
        _ => "GC finding",
    }
}

pub fn fmt_ms(ms: f64) -> String {
    let a = ms.abs();
    if a > 0.0 && a < 0.1 {
        format!("{:.0} µs", ms * 1000.0)
    } else if a < 10.0 {
        format!("{ms:.2} ms")
    } else if a < 1000.0 {
        format!("{ms:.0} ms")
    } else if a < 60_000.0 {
        format!("{:.2} s", ms / 1000.0)
    } else {
        let s = (a / 1000.0).round() as u64;
        let (d, h, m, sec) = (s / 86_400, (s % 86_400) / 3600, (s % 3600) / 60, s % 60);
        if d > 0 {
            format!("{d}d {h}h")
        } else if h > 0 {
            format!("{h}h {m:02}m")
        } else {
            format!("{m}m {sec:02}s")
        }
    }
}

pub fn fmt_kb(kb: f64) -> String {
    let units = ["KB", "MB", "GB", "TB"];
    let mut v = kb;
    let mut u = 0;
    while v.abs() >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 || v.abs() >= 100.0 {
        format!("{v:.0} {}", units[u])
    } else {
        format!("{v:.1} {}", units[u])
    }
}

fn fmt_pct(p: Option<f64>) -> String {
    p.map_or("—".into(), |v| if v >= 99.0 && v < 100.0 { format!("{v:.2}%") } else { format!("{v:.1}%") })
}

/// `count=3;max_ms=12.5` → `count 3 · max 12.5 ms`.
fn humanize_detail(detail: &str) -> String {
    detail
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| {
            let (label, value) = match k.strip_suffix("_ms") {
                Some(base) => (base.to_string(), v.parse::<f64>().map(fmt_ms).unwrap_or_else(|_| v.into())),
                None => match k.strip_suffix("_pct") {
                    Some(base) => (base.to_string(), format!("{v}%")),
                    None => (k.to_string(), v.to_string()),
                },
            };
            format!("{} {value}", label.replace('_', " "))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Unicode sparkline of `values`, downsampled to `width` buckets (max per bucket).
pub fn sparkline(values: &[f64], width: usize) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if values.is_empty() || width == 0 {
        return String::new();
    }
    let n = values.len().min(width);
    let buckets: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b) = (i * values.len() / n, ((i + 1) * values.len() / n).max(i * values.len() / n + 1));
            values[a..b].iter().copied().fold(f64::MIN, f64::max)
        })
        .collect();
    let (lo, hi) = buckets.iter().fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    buckets
        .iter()
        .map(|&v| {
            let t = if hi > lo { (v - lo) / (hi - lo) } else { 0.5 };
            BARS[((t * 7.0).round() as usize).min(7)]
        })
        .collect()
}

fn severity_allowed(f: &GcFinding, filter: Option<SeverityFilter>) -> bool {
    let rank = |s: &str| match s {
        "critical" => 0,
        "warning" => 1,
        _ => 2,
    };
    match filter {
        None => true,
        Some(SeverityFilter::Critical) => rank(&f.severity) <= 0,
        Some(SeverityFilter::Warning) => rank(&f.severity) <= 1,
        Some(SeverityFilter::Info) => true,
    }
}

fn collector_label(c: Collector) -> &'static str {
    c.label()
}

fn event_line(e: &GcEvent) -> String {
    let id = e.gc_id.map_or(String::new(), |i| format!("GC({i}) "));
    let cause = e.cause.as_deref().map_or(String::new(), |c| format!("  {c}"));
    let heap = e.heap.map_or(String::new(), |h| format!("  {} → {}", fmt_kb(h.before_kb), fmt_kb(h.after_kb)));
    let mut flags = Vec::new();
    if e.flags.evacuation_failure {
        flags.push("evacuation failure");
    }
    if e.flags.promotion_failed {
        flags.push("promotion failed");
    }
    if e.flags.concurrent_mode_failure {
        flags.push("concurrent mode failure");
    }
    if e.flags.degenerated {
        flags.push("degenerated");
    }
    let flags = if flags.is_empty() { String::new() } else { format!("  [{}]", flags.join(", ")) };
    format!("{:>10}  @{:>10}  {id}{}{cause}{heap}{flags}", fmt_ms(e.duration_ms), fmt_ms(e.time_ms), e.name)
}

/// Text report for every JVM group.
pub fn render_gc_text(report: &GcReport, stitched: &Stitched, labels: &[String], opts: &CliOptions) -> String {
    let style = &Style::from_opts(opts);
    let mut out = String::new();
    let limit = opts.limit.unwrap_or(10);
    for (g, group) in report.groups.iter().zip(&stitched.groups) {
        let name = labels.get(g.files[0]).map_or(g.name.as_str(), String::as_str);
        let segs = if g.files.len() > 1 { format!(" (+{} segments)", g.files.len() - 1) } else { String::new() };
        let version = g.jvm.version.as_deref().map_or(String::new(), |v| format!(" · JDK {v}"));
        out.push_str(&style.bold(&format!(
            "GC log: {name}{segs} · {}{version} · {} · {} events\n",
            collector_label(g.collector),
            fmt_ms(g.stats.span_ms),
            g.event_count
        )));
        for n in &g.notes {
            out.push_str(&format!("  note: {:?} {} ({})\n", n.kind, labels.get(n.file_index).map_or("?", String::as_str), n.detail));
        }
        if g.event_count == 0 {
            out.push_str("  no GC events found\n\n");
            continue;
        }

        if opts.sections.contains(&Section::Findings) {
            out.push_str(&style.bold("Findings\n"));
            let shown: Vec<&GcFinding> = g.findings.iter().filter(|f| severity_allowed(f, opts.severity)).collect();
            if shown.is_empty() {
                out.push_str("  ✓ no GC problems found\n");
            }
            for f in shown {
                out.push_str(&format!(
                    "  {} {} {:<8} {}\n      {}\n",
                    style.sev_mark(&f.severity),
                    f.rule,
                    f.severity,
                    rule_title(&f.rule),
                    humanize_detail(&f.detail)
                ));
            }
        }

        let s = &g.stats;
        if opts.sections.contains(&Section::Summary) {
            out.push_str(&style.bold("Summary\n"));
            let full = s.kinds.iter().find(|k| k.kind == GcKind::Full).map_or(0, |k| k.count);
            out.push_str(&format!("  throughput {}   total pause {}   pauses {}   full GC {}\n", fmt_pct(s.throughput_pct), fmt_ms(s.total_pause_ms), s.pause_count, full));
            if let Some(p) = s.pause {
                out.push_str(&format!(
                    "  pause P50 {}  P95 {}  P99 {}  max {}\n",
                    fmt_ms(p.p50),
                    fmt_ms(p.p95),
                    fmt_ms(p.p99),
                    fmt_ms(p.max)
                ));
            }
            if let Some(a) = s.allocation {
                out.push_str(&format!("  allocation {:.1} MB/s (peak {:.1})", a.avg_mb_per_s, a.peak_mb_per_s));
                if let Some(pr) = s.promotion {
                    out.push_str(&format!("   promotion {:.1} MB/s", pr.avg_mb_per_s));
                }
                out.push('\n');
            }
            let max = g.jvm.heap_max_kb.map_or(String::new(), |m| format!(" of {}", fmt_kb(m)));
            out.push_str(&format!("  heap after GC peak {}{max}\n", fmt_pct(s.heap_after_peak_pct)));
            let heap_after: Vec<f64> = group.log.events.iter().filter_map(|e| e.heap.map(|h| h.after_kb)).collect();
            if heap_after.len() > 1 {
                out.push_str(&format!("  heap after GC  {}\n", sparkline(&heap_after, 60)));
            }
        }

        if opts.sections.contains(&Section::Causes) && !s.causes.is_empty() {
            out.push_str(&style.bold("Causes\n"));
            for c in s.causes.iter().take(limit) {
                out.push_str(&format!("  {:<34} {:>6}×  total {:>10}  max {:>10}\n", c.cause, c.count, fmt_ms(c.total_pause_ms), fmt_ms(c.max_pause_ms)));
            }
        }

        if opts.sections.contains(&Section::Pauses) {
            out.push_str(&style.bold(&format!("Longest pauses (top {limit})\n")));
            let mut idx: Vec<usize> = (0..group.log.events.len()).filter(|&i| group.log.events[i].is_pause).collect();
            idx.sort_unstable_by(|&a, &b| group.log.events[b].duration_ms.total_cmp(&group.log.events[a].duration_ms).then(a.cmp(&b)));
            for i in idx.into_iter().take(limit) {
                out.push_str(&format!("  {}\n", event_line(&group.log.events[i])));
            }
        }

        if opts.sections.contains(&Section::Events) {
            let n = opts.limit.unwrap_or(usize::MAX);
            out.push_str(&style.bold(&format!("Events ({})\n", group.log.events.len())));
            for e in group.log.events.iter().take(n) {
                out.push_str(&format!("  {}\n", event_line(e)));
            }
        }
        out.push('\n');
    }
    out
}

/// `1` when any group has a warning or critical finding.
pub fn exit_for_gc(report: &GcReport) -> CliExit {
    let bad = report.groups.iter().flat_map(|g| &g.findings).any(|f| f.severity == "critical" || f.severity == "warning");
    if bad {
        CliExit::Problems
    } else {
        CliExit::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_gc_formatting_helpers() {
        assert_eq!(fmt_ms(0.005), "5 µs");
        assert_eq!(fmt_ms(4.987), "4.99 ms");
        assert_eq!(fmt_ms(1420.0), "1.42 s");
        assert_eq!(fmt_ms(334_711_118.0), "3d 20h");
        assert_eq!(fmt_kb(5_592_576.0), "5.3 GB");
        assert_eq!(humanize_detail("count=2;max_ms=1420;peak_pct=99.5"), "count 2 · max 1.42 s · peak 99.5%");
        assert_eq!(sparkline(&[0.0, 1.0, 2.0, 3.0], 4), "▁▃▆█");
        assert_eq!(sparkline(&[], 10), "");
        assert_eq!(sparkline(&(0..1000).map(f64::from).collect::<Vec<_>>(), 10).chars().count(), 10);
    }
}
