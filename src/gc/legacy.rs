//! JDK 8 `-XX:+PrintGCDetails` parser (feat-069): Parallel, Serial, CMS and G1.
//!
//! Output is assembled into *records*: a record starts at a line beginning with
//! an optional date stamp / uptime and `[GC` or `[Full GC`, and continues across
//! lines (tenuring distribution, G1 phase details, `PrintReferenceGC` output)
//! until its `[Times: …]` line. Concurrent-phase markers (`[CMS-concurrent-…]`,
//! `[GC concurrent-…]`) can be interleaved anywhere, so they are extracted from
//! every line first. Record text is normalized (inner timestamps and timed
//! sub-phases removed) before generations, heap, duration and CPU are read.

use regex::{Captures, Regex};

use super::model::{
    Collector, CpuTimes, EventFlags, GcEvent, GcKind, GcLogFormat, JvmFact, JvmInfo, MemChange, ParsedGcLog,
    MAX_UNPARSED_SAMPLES,
};
use super::units::{mem_change_regex, parse_decimal, parse_iso_epoch_ms, parse_mem_change, parse_size_kb};

const DATE: &str = r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d+[+-]\d{4}";

struct Patterns {
    record_start: Regex,
    stamp: Regex,
    inner_stamp: Regex,
    cms_concurrent: Regex,
    cms_concurrent_start: Regex,
    g1_concurrent_end: Regex,
    g1_concurrent_other: Regex,
    timed_subphase: Regex,
    times: Regex,
    secs: Regex,
    generation: Regex,
    occupancy: Regex,
    g1_sizes: Regex,
    rotation: Regex,
    mem: Regex,
}

impl Patterns {
    fn new() -> Self {
        let stamp = format!(r"(?:(?P<date>{DATE}): )?(?:(?P<up>\d+[.,]\d+): )?");
        Self {
            record_start: Regex::new(&format!(r"^{stamp}\[(?:GC|Full GC)\b")).unwrap(),
            stamp: Regex::new(&format!(r"^{stamp}")).unwrap(),
            inner_stamp: Regex::new(&format!(r"(?:{DATE}: )?\d+[.,]\d+: \[")).unwrap(),
            cms_concurrent: Regex::new(&format!(
                r"{stamp}\[CMS-concurrent-(?P<phase>[a-z-]+): (?P<cpu>[\d.]+)/(?P<wall>[\d.]+) secs\](?: \[Times: [^\]]*\])?"
            ))
            .unwrap(),
            cms_concurrent_start: Regex::new(&format!(r"(?:{DATE}: )?(?:\d+[.,]\d+: )?\[CMS-concurrent-[a-z-]+-start\]")).unwrap(),
            g1_concurrent_end: Regex::new(&format!(r"{stamp}\[GC concurrent-(?P<phase>[a-z-]+)-end, (?P<secs>[\d.]+) secs\]")).unwrap(),
            g1_concurrent_other: Regex::new(&format!(
                r"(?:{DATE}: )?(?:\d+[.,]\d+: )?\[GC concurrent-[a-z-]+(?:-start|-abort)\]"
            ))
            .unwrap(),
            // Timed sub-phases without a size change: reference processing, remark steps, …
            timed_subphase: Regex::new(r"\[[^\[\]]*?, [\d.]+ secs\]").unwrap(),
            times: Regex::new(r"\[Times: user=([\d.,]+) sys=([\d.,]+), real=([\d.,]+) secs\]").unwrap(),
            secs: Regex::new(r", ([\d.,]+) secs\]").unwrap(),
            generation: Regex::new(
                r"\[(?P<gen>PSYoungGen|ASPSYoungGen|ParOldGen|ASParOldGen|PSOldGen|DefNew|ParNew|Tenured|CMS|Metaspace|PSPermGen|CMS Perm|Perm)(?P<note> \([^)]*\))?: (?P<body>[^\[\]]*)\]",
            )
            .unwrap(),
            occupancy: Regex::new(r"(\d+(?:\.\d+)?[BKMG])\((\d+(?:\.\d+)?[BKMG])\)").unwrap(),
            g1_sizes: Regex::new(
                r"Eden: (\S+?)\((\S+?)\)->(\S+?)\((\S+?)\) Survivors: (\S+?)->(\S+?) Heap: (\S+?)\((\S+?)\)->(\S+?)\((\S+?)\)",
            )
            .unwrap(),
            rotation: Regex::new(r"^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d GC log file (?:created (?P<created>.+)|has reached the maximum size\. Saved as (?P<saved>.+))$").unwrap(),
            mem: mem_change_regex(),
        }
    }
}

/// Uptime (ms) and wall clock (epoch ms) of a matched stamp.
fn stamp_times(c: &Captures) -> (Option<f64>, Option<f64>) {
    let up = c.name("up").and_then(|m| parse_decimal(m.as_str())).map(|s| s * 1000.0);
    let wall = c.name("date").and_then(|m| parse_iso_epoch_ms(m.as_str()));
    (up, wall)
}

/// The parenthesized groups right after `keyword` (handles nested `System.gc()`).
fn paren_groups_after<'a>(text: &'a str, keyword: &str) -> Vec<&'a str> {
    let Some(pos) = text.find(keyword) else { return Vec::new() };
    let bytes = text.as_bytes();
    let mut i = pos + keyword.len();
    let mut out = Vec::new();
    loop {
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'(' {
            break;
        }
        let start = i + 1;
        let mut depth = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        out.push(&text[start..i]);
        i += 1;
    }
    out
}

/// Builder of one record (start line + continuation lines).
struct Record {
    line: usize,
    uptime_ms: Option<f64>,
    wall_ms: Option<f64>,
    lines: Vec<String>,
}

fn parse_record(p: &Patterns, r: &Record, collector: &mut Collector) -> Option<GcEvent> {
    let first = r.lines.first()?;
    let is_g1_pause = first.contains("[GC pause");
    let g1_style = is_g1_pause || first.contains("[GC remark") || first.contains("[GC cleanup") || r.lines.iter().any(|l| l.contains("Eden: ") && l.contains("Heap: "));
    // G1 detail lines are per-phase timings; only the header, the Eden/Heap summary and Times matter.
    let raw: String = if g1_style {
        r.lines
            .iter()
            .enumerate()
            .filter(|(i, l)| *i == 0 || l.contains("Eden: ") || l.contains("[Times: "))
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        r.lines
            .iter()
            .filter(|l| !l.starts_with("Desired survivor size") && !l.trim_start().starts_with("- age"))
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("")
    };
    // Drop the record's own stamp, then inner stamps before nested brackets.
    let body = p.stamp.replace(&raw, "").into_owned();
    let body = p.inner_stamp.replace_all(&body, "[").into_owned();
    let cpu = p.times.captures(&body).map(|c| CpuTimes {
        user_ms: parse_decimal(&c[1]).unwrap_or(0.0) * 1000.0,
        sys_ms: parse_decimal(&c[2]).unwrap_or(0.0) * 1000.0,
        real_ms: parse_decimal(&c[3]).unwrap_or(0.0) * 1000.0,
    });
    let body = p.times.replace_all(&body, "").into_owned();

    let mut flags = EventFlags::default();
    flags.evacuation_failure = body.contains("to-space exhausted") || body.contains("to-space overflow");
    flags.promotion_failed = body.contains("promotion failed");
    flags.concurrent_mode_failure = body.contains("concurrent mode failure");

    // Duration: G1 headers state it first; otherwise the outermost (last) `, N secs]`.
    let duration_ms = if g1_style {
        p.secs.captures(&body).and_then(|c| parse_decimal(&c[1]))
    } else {
        // Remove timed sub-phases that have no size change (reference processing, remark steps).
        let stripped = p.timed_subphase.replace_all(&body, |c: &Captures| {
            let m = &c[0];
            if m.contains("->") || m.contains("K(") { m.to_string() } else { String::new() }
        });
        p.secs.captures_iter(&stripped).last().and_then(|c| parse_decimal(&c[1]))
    }? * 1000.0;

    // Generations (Parallel / Serial / CMS) and metaspace.
    let mut young = None;
    let mut old = None;
    let mut metaspace = None;
    let mut old_collected = false;
    for c in p.generation.captures_iter(&body) {
        let change = parse_mem_change(&p.mem, &c["body"]);
        match &c["gen"] {
            "PSYoungGen" | "ASPSYoungGen" | "DefNew" | "ParNew" => young = change.or(young),
            "ParOldGen" | "ASParOldGen" | "PSOldGen" | "Tenured" | "CMS" => {
                old_collected = true;
                old = change.or(old);
            }
            "Metaspace" | "PSPermGen" | "CMS Perm" | "Perm" => metaspace = change.or(metaspace),
            _ => {}
        }
        if *collector == Collector::Unknown {
            *collector = match &c["gen"] {
                "PSYoungGen" | "ASPSYoungGen" | "ParOldGen" | "PSOldGen" => Collector::Parallel,
                "DefNew" | "Tenured" => Collector::Serial,
                "ParNew" | "CMS" => Collector::Cms,
                _ => Collector::Unknown,
            };
        }
    }
    let outer = p.generation.replace_all(&body, "").into_owned();
    let mut heap = parse_mem_change(&p.mem, &outer);

    if let Some(c) = p.g1_sizes.captures(&body) {
        let kb = |i: usize| parse_size_kb(&c[i]);
        if let (Some(eb), Some(ebc), Some(ea), Some(eac), Some(sb), Some(sa), Some(hb), Some(hbc), Some(ha), Some(hac)) =
            (kb(1), kb(2), kb(3), kb(4), kb(5), kb(6), kb(7), kb(8), kb(9), kb(10))
        {
            let y = MemChange { before_kb: eb + sb, after_kb: ea + sa, capacity_before_kb: Some(ebc + sb), capacity_after_kb: Some(eac + sa) };
            heap = Some(MemChange { before_kb: hb, after_kb: ha, capacity_before_kb: Some(hbc), capacity_after_kb: Some(hac) });
            old = Some(MemChange {
                before_kb: (hb - y.before_kb).max(0.0),
                after_kb: (ha - y.after_kb).max(0.0),
                capacity_before_kb: None,
                capacity_after_kb: None,
            });
            young = Some(y);
        }
        *collector = Collector::G1;
    }

    // Kind, name, cause.
    let (kind, name, cause): (GcKind, &str, Option<String>);
    if is_g1_pause {
        *collector = Collector::G1;
        let groups = paren_groups_after(&body, "GC pause");
        let has = |g: &str| groups.iter().any(|x| *x == g);
        cause = groups.first().map(|s| s.to_string());
        if has("mixed") {
            (kind, name) = (GcKind::Mixed, "Pause Young (Mixed)");
        } else if has("initial-mark") {
            (kind, name) = (GcKind::Young, "Pause Young (Concurrent Start)");
        } else {
            (kind, name) = (GcKind::Young, "Pause Young (Normal)");
        }
    } else if body.contains("[GC remark") {
        (kind, name, cause) = (GcKind::Remark, "Pause Remark", None);
    } else if body.contains("[GC cleanup") {
        (kind, name, cause) = (GcKind::Cleanup, "Pause Cleanup", None);
    } else if body.contains("[GC (CMS Initial Mark)") {
        (kind, name, cause) = (GcKind::InitialMark, "Pause Initial Mark", None);
        // `[1 CMS-initial-mark: 13138K(32768K)] 14353K(47552K)` reports occupancy, not a change.
        let occ: Vec<_> = p.occupancy.captures_iter(&body).collect();
        let point = |c: &Captures| -> Option<MemChange> {
            let v = parse_size_kb(&c[1])?;
            Some(MemChange { before_kb: v, after_kb: v, capacity_before_kb: parse_size_kb(&c[2]), capacity_after_kb: parse_size_kb(&c[2]) })
        };
        old = occ.first().and_then(point);
        heap = occ.get(1).and_then(point);
    } else if body.contains("[GC (CMS Final Remark)") {
        (kind, name, cause) = (GcKind::Remark, "Pause Remark", None);
        let occ: Vec<_> = p.occupancy.captures_iter(&body).filter(|c| !c[0].contains(' ')).collect();
        let point = |c: &Captures| -> Option<MemChange> {
            let v = parse_size_kb(&c[1])?;
            Some(MemChange { before_kb: v, after_kb: v, capacity_before_kb: parse_size_kb(&c[2]), capacity_after_kb: parse_size_kb(&c[2]) })
        };
        heap = occ.last().and_then(point);
    } else if body.contains("[Full GC") {
        (kind, name) = (GcKind::Full, "Pause Full");
        cause = paren_groups_after(&body, "[Full GC").first().map(|s| s.to_string());
    } else if body.contains("[GC") {
        cause = paren_groups_after(&body, "[GC").first().map(|s| s.to_string());
        // A `[GC` record that also collected the old generation (promotion-failure fallback) is a full collection.
        if old_collected {
            (kind, name) = (GcKind::Full, "Pause Full");
        } else {
            (kind, name) = (GcKind::Young, "Pause Young");
        }
    } else {
        return None;
    }

    Some(GcEvent {
        gc_id: None,
        time_ms: 0.0,
        uptime_ms: r.uptime_ms,
        wall_ms: r.wall_ms,
        kind,
        is_pause: true,
        name: name.to_string(),
        cause,
        duration_ms,
        heap,
        young,
        old,
        humongous: None,
        metaspace,
        cpu,
        flags,
        file_index: 0,
        line: r.line,
    })
}

fn flag_value<'a>(command_line: &'a str, name: &str) -> Option<&'a str> {
    command_line.split_whitespace().find_map(|f| f.strip_prefix(&format!("-XX:{name}=")))
}

fn collector_from_flags(command_line: &str) -> Collector {
    let on = |f: &str| command_line.split_whitespace().any(|x| x == format!("-XX:+{f}"));
    if on("UseG1GC") {
        Collector::G1
    } else if on("UseConcMarkSweepGC") {
        Collector::Cms
    } else if on("UseSerialGC") {
        Collector::Serial
    } else if on("UseParallelGC") || on("UseParallelOldGC") {
        Collector::Parallel
    } else {
        Collector::Unknown
    }
}

/// Parse a JDK 8 `-XX:+PrintGCDetails` log.
pub fn parse(text: &str) -> ParsedGcLog {
    let p = Patterns::new();
    let mut jvm = JvmInfo::default();
    let mut collector = Collector::Unknown;
    let mut events: Vec<GcEvent> = Vec::new();
    let mut unparsed_count = 0;
    let mut unparsed_samples: Vec<(usize, String)> = Vec::new();
    let mut line_count = 0;
    let (mut first_up, mut last_up, mut first_wall, mut last_wall) = (None, None, None, None);
    let (mut rotation_created, mut rotation_saved_as) = (None, None);
    let mut current: Option<Record> = None;
    let mut in_exit_summary = false;

    let mut note_time = |up: Option<f64>, wall: Option<f64>| {
        if let Some(u) = up {
            first_up.get_or_insert(u);
            last_up = Some(u);
        }
        if let Some(w) = wall {
            first_wall.get_or_insert(w);
            last_wall = Some(w);
        }
    };

    let flush = |rec: Option<Record>, events: &mut Vec<GcEvent>, collector: &mut Collector, unparsed: &mut usize, samples: &mut Vec<(usize, String)>| {
        if let Some(r) = rec {
            match parse_record(&p, &r, collector) {
                Some(e) => events.push(e),
                None => {
                    *unparsed += 1;
                    if samples.len() < MAX_UNPARSED_SAMPLES {
                        samples.push((r.line, r.lines.join(" ").chars().take(400).collect()));
                    }
                }
            }
        }
    };

    for (idx, raw_line) in text.lines().enumerate() {
        line_count += 1;
        let line_no = idx + 1;
        let mut line = raw_line.trim_end_matches('\r').to_string();
        if line.trim().is_empty() {
            continue;
        }

        // Header and rotation markers.
        if (line.starts_with("Java HotSpot") || line.starts_with("OpenJDK")) && line.contains(" VM (") {
            if let Some(v) = line.split("JRE (").nth(1).and_then(|r| r.split(')').next()) {
                jvm.version = Some(v.to_string());
            }
            jvm.facts.push(JvmFact { key: "VM".into(), value: line.clone() });
            continue;
        }
        if let Some(rest) = line.strip_prefix("Memory: ") {
            jvm.facts.push(JvmFact { key: "Memory".into(), value: rest.to_string() });
            continue;
        }
        if let Some(rest) = line.strip_prefix("CommandLine flags: ") {
            let flags = rest.trim().to_string();
            jvm.heap_max_kb = flag_value(&flags, "MaxHeapSize").and_then(|v| v.parse::<f64>().ok()).map(|b| b / 1024.0);
            jvm.heap_initial_kb = flag_value(&flags, "InitialHeapSize").and_then(|v| v.parse::<f64>().ok()).map(|b| b / 1024.0);
            jvm.pause_target_ms = flag_value(&flags, "MaxGCPauseMillis").and_then(|v| v.parse().ok());
            jvm.region_size_kb = flag_value(&flags, "G1HeapRegionSize").and_then(|v| v.parse::<f64>().ok()).map(|b| b / 1024.0);
            let from_flags = collector_from_flags(&flags);
            if from_flags != Collector::Unknown {
                collector = from_flags;
            }
            jvm.facts.push(JvmFact { key: "CommandLine flags".into(), value: flags.clone() });
            jvm.command_line = Some(flags);
            continue;
        }
        if let Some(c) = p.rotation.captures(&line) {
            if let Some(m) = c.name("created") {
                rotation_created = Some(m.as_str().to_string());
            }
            if let Some(m) = c.name("saved") {
                rotation_saved_as = Some(m.as_str().to_string());
            }
            continue;
        }

        // Concurrent phases may be interleaved anywhere in a line.
        if line.contains("concurrent-") {
            for c in p.cms_concurrent.captures_iter(&line) {
                let (up, wall) = stamp_times(&c);
                let wall_s: f64 = c["wall"].parse().unwrap_or(0.0);
                note_time(up, wall);
                let dur = wall_s * 1000.0;
                events.push(GcEvent {
                    gc_id: None,
                    time_ms: 0.0,
                    uptime_ms: up.map(|u| (u - dur).max(0.0)),
                    wall_ms: wall.map(|w| w - dur),
                    kind: GcKind::Concurrent,
                    is_pause: false,
                    name: format!("Concurrent {}", c["phase"].replace('-', " ")),
                    cause: None,
                    duration_ms: dur,
                    heap: None,
                    young: None,
                    old: None,
                    humongous: None,
                    metaspace: None,
                    cpu: None,
                    flags: EventFlags::default(),
                    file_index: 0,
                    line: line_no,
                });
            }
            for c in p.g1_concurrent_end.captures_iter(&line) {
                let (up, wall) = stamp_times(&c);
                note_time(up, wall);
                let dur = c["secs"].parse::<f64>().unwrap_or(0.0) * 1000.0;
                events.push(GcEvent {
                    gc_id: None,
                    time_ms: 0.0,
                    uptime_ms: up.map(|u| (u - dur).max(0.0)),
                    wall_ms: wall.map(|w| w - dur),
                    kind: GcKind::Concurrent,
                    is_pause: false,
                    name: format!("Concurrent {}", c["phase"].replace('-', " ")),
                    cause: None,
                    duration_ms: dur,
                    heap: None,
                    young: None,
                    old: None,
                    humongous: None,
                    metaspace: None,
                    cpu: None,
                    flags: EventFlags::default(),
                    file_index: 0,
                    line: line_no,
                });
            }
            line = p.cms_concurrent.replace_all(&line, "").into_owned();
            line = p.cms_concurrent_start.replace_all(&line, "").into_owned();
            line = p.g1_concurrent_end.replace_all(&line, "").into_owned();
            line = p.g1_concurrent_other.replace_all(&line, "").into_owned();
            if line.trim().is_empty() {
                continue;
            }
        }

        if let Some(c) = p.record_start.captures(&line) {
            flush(current.take(), &mut events, &mut collector, &mut unparsed_count, &mut unparsed_samples);
            in_exit_summary = false;
            let (up, wall) = stamp_times(&c);
            note_time(up, wall);
            let done = line.contains("[Times: ");
            let rec = Record { line: line_no, uptime_ms: up, wall_ms: wall, lines: vec![line] };
            if done {
                flush(Some(rec), &mut events, &mut collector, &mut unparsed_count, &mut unparsed_samples);
            } else {
                current = Some(rec);
            }
            continue;
        }
        if line == "Heap" || line.starts_with("{Heap") || line.starts_with("Heap after") {
            flush(current.take(), &mut events, &mut collector, &mut unparsed_count, &mut unparsed_samples);
            in_exit_summary = true;
            continue;
        }
        if let Some(rec) = current.as_mut() {
            let done = line.contains("[Times: ");
            rec.lines.push(line);
            if done {
                flush(current.take(), &mut events, &mut collector, &mut unparsed_count, &mut unparsed_samples);
            }
            continue;
        }
        if in_exit_summary || line.starts_with(' ') || line.starts_with('}') {
            // Heap summary at exit / PrintHeapAtGC blocks.
            continue;
        }
        unparsed_count += 1;
        if unparsed_samples.len() < MAX_UNPARSED_SAMPLES {
            unparsed_samples.push((line_no, line));
        }
    }
    flush(current.take(), &mut events, &mut collector, &mut unparsed_count, &mut unparsed_samples);

    if collector == Collector::Unknown && events.iter().any(|e| e.name.starts_with("Pause Young (")) {
        collector = Collector::G1;
    }
    super::unified::assign_time_axis(&mut events);
    let missing_timestamps = !events.is_empty() && events.iter().all(|e| e.uptime_ms.is_none() && e.wall_ms.is_none());
    jvm.facts.insert(0, JvmFact { key: "Collector".into(), value: collector.label().into() });

    ParsedGcLog {
        format: GcLogFormat::Legacy,
        collector,
        jvm,
        events,
        line_count,
        unparsed_count,
        unparsed_samples,
        pid: None,
        missing_timestamps,
        first_line_uptime_ms: first_up,
        last_line_uptime_ms: last_up,
        first_line_wall_ms: first_wall,
        last_line_wall_ms: last_wall,
        rotation_created,
        rotation_saved_as,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paren_groups_nested() {
        assert_eq!(paren_groups_after("[Full GC (System.gc()) [PSYoungGen", "[Full GC"), vec!["System.gc()"]);
        assert_eq!(
            paren_groups_after("[GC pause (G1 Humongous Allocation) (young) (initial-mark), 0.1 secs]", "GC pause"),
            vec!["G1 Humongous Allocation", "young", "initial-mark"]
        );
        assert!(paren_groups_after("[GC [PSYoungGen: 1K->0K(2K)]", "[GC").is_empty());
    }
}
