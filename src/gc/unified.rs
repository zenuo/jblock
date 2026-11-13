//! JDK 9+ unified logging (`-Xlog:gc*`) parser (feat-068).
//!
//! Decorators are recognized by content, not position, so any combination of
//! `time`, `utctime`, `uptime`, `timemillis`, `uptimemillis`, `timenanos`,
//! `uptimenanos`, `hostname`, `pid`, `tid`, `level` and `tags` works. Multi-line
//! events are aggregated by their `GC(n)` id: heap/metaspace lines precede the
//! summary line and attach to the next event of that id, `gc,cpu` lines follow
//! it and attach to the previous one.

use std::collections::HashMap;

use regex::Regex;

use super::model::{
    Collector, CpuTimes, EventFlags, GcEvent, GcKind, GcLogFormat, JvmFact, JvmInfo, MemChange,
    ParsedGcLog, MAX_UNPARSED_SAMPLES,
};
use super::units::{mem_change_regex, parse_decimal, parse_iso_epoch_ms, parse_mem_change, parse_size_kb};

/// Decorations and message of one unified-logging line.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Decorated<'a> {
    pub uptime_ms: Option<f64>,
    pub wall_ms: Option<f64>,
    pub pid: Option<u64>,
    pub level: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub msg: &'a str,
}

const LEVELS: [&str; 5] = ["trace", "debug", "info", "warning", "error"];

fn number_with_suffix<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let num = text.strip_suffix(suffix)?;
    if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        Some(num)
    } else {
        None
    }
}

/// Split the leading `[..][..]` decorators from a unified-logging line.
///
/// Returns `None` when the line does not start with recognizable decorators
/// (e.g. JDK 8 output, application stdout).
pub(crate) fn split_decorators(line: &str) -> Option<Decorated<'_>> {
    if !line.starts_with('[') {
        // `-Xlog:gc:file:none` writes bare messages.
        if line.starts_with("GC(") || line.starts_with("Using ") {
            return Some(Decorated { msg: line, ..Default::default() });
        }
        return None;
    }
    let mut out = Decorated::default();
    let mut rest = line;
    let mut recognized = 0;
    let mut numeric_ids = 0;
    while let Some(inner_start) = rest.strip_prefix('[') {
        let end = inner_start.find(']')?;
        let raw = &inner_start[..end];
        let group = raw.trim();
        rest = &inner_start[end + 1..];
        if group.len() >= 19 && group.as_bytes().get(10) == Some(&b'T') {
            if let Some(ms) = parse_iso_epoch_ms(group) {
                out.wall_ms = Some(ms);
                recognized += 1;
                continue;
            }
        }
        if let Some(num) = number_with_suffix(group, "ms") {
            let v = parse_decimal(num)?;
            if v >= 1e11 {
                out.wall_ms = Some(v);
            } else {
                out.uptime_ms = Some(v);
            }
            recognized += 1;
        } else if let Some(num) = number_with_suffix(group, "ns") {
            let v = parse_decimal(num)?;
            if v >= 1e17 {
                out.wall_ms = Some(v / 1e6);
            } else {
                out.uptime_ms = Some(v / 1e6);
            }
            recognized += 1;
        } else if let Some(num) = number_with_suffix(group, "s") {
            out.uptime_ms = Some(parse_decimal(num)? * 1000.0);
            recognized += 1;
        } else if LEVELS.contains(&group) {
            out.level = Some(group);
            recognized += 1;
        } else if !group.is_empty() && group.chars().all(|c| c.is_ascii_digit()) {
            if numeric_ids == 0 {
                out.pid = group.parse().ok();
            }
            numeric_ids += 1;
        } else if !group.is_empty()
            && group.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ',' || c == '_')
            && group.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        {
            // Tags are always the last decorator; a hostname may look similar but comes earlier.
            out.tags = Some(group);
            recognized += 1;
        }
        // Anything else (hostname) is ignored.
    }
    if recognized == 0 {
        return None;
    }
    out.msg = rest.strip_prefix(' ').unwrap_or(rest);
    Some(out)
}

/// Strip a leading `GC(n) ` from a message.
fn split_gc_id(msg: &str) -> (Option<u64>, &str) {
    if let Some(rest) = msg.strip_prefix("GC(") {
        if let Some(end) = rest.find(')') {
            if let Ok(id) = rest[..end].parse() {
                return (Some(id), rest[end + 1..].trim_start());
            }
        }
    }
    (None, msg)
}

/// Strip a trailing ` 1.234ms` / ` 1.234s` duration.
fn split_trailing_duration(msg: &str) -> (Option<f64>, &str) {
    let trimmed = msg.trim_end();
    if let Some(pos) = trimmed.rfind(' ') {
        let token = &trimmed[pos + 1..];
        if let Some(num) = number_with_suffix(token, "ms") {
            if let Some(v) = parse_decimal(num) {
                return (Some(v), trimmed[..pos].trim_end());
            }
        } else if let Some(num) = number_with_suffix(token, "s") {
            if let Some(v) = parse_decimal(num) {
                return (Some(v * 1000.0), trimmed[..pos].trim_end());
            }
        }
    }
    (None, trimmed)
}

/// Split `Pause Young (Normal) (G1 Evacuation Pause)` into base and parenthesized groups.
/// Handles nested parentheses such as `(System.gc())`.
fn split_paren_groups(text: &str) -> (&str, Vec<&str>) {
    let base_end = text.find(" (").unwrap_or(text.len());
    let base = text[..base_end].trim();
    let mut groups = Vec::new();
    let bytes = text.as_bytes();
    let mut i = base_end;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let start = i + 1;
            let mut depth = 1;
            let mut j = start;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            groups.push(&text[start..j.saturating_sub(1).max(start)]);
            i = j;
        } else {
            i += 1;
        }
    }
    (base, groups)
}

const G1_YOUNG_SUBTYPES: [&str; 4] = ["Normal", "Concurrent Start", "Prepare Mixed", "Mixed"];

fn kind_for(base: &str, subtype: Option<&str>) -> GcKind {
    match base {
        "Pause Young" if subtype == Some("Mixed") => GcKind::Mixed,
        "Pause Young" => GcKind::Young,
        "Pause Mixed" => GcKind::Mixed,
        "Pause Full" => GcKind::Full,
        "Pause Remark" => GcKind::Remark,
        "Pause Cleanup" => GcKind::Cleanup,
        "Pause Initial Mark" => GcKind::InitialMark,
        b if b.starts_with("Pause ") => GcKind::OtherPause,
        _ => GcKind::Concurrent,
    }
}

/// G1 region counts for one event, converted to KiB once the region size is known.
#[derive(Debug, Default, Clone, Copy)]
struct Regions {
    eden: Option<(f64, f64, Option<f64>)>,
    survivor: Option<(f64, f64, Option<f64>)>,
    old: Option<(f64, f64)>,
    humongous: Option<(f64, f64)>,
}

impl Regions {
    fn is_empty(&self) -> bool {
        self.eden.is_none() && self.survivor.is_none() && self.old.is_none() && self.humongous.is_none()
    }
}

/// Per-`GC(n)` state collected before the summary line.
#[derive(Debug, Default)]
struct Pending {
    start_uptime_ms: Option<f64>,
    start_wall_ms: Option<f64>,
    /// Concurrent cycles overlap their own Remark/Cleanup pauses under the same id.
    cycle_start_uptime_ms: Option<f64>,
    cycle_start_wall_ms: Option<f64>,
    /// ZGC `Garbage Collection` / `Minor Collection` / `Major Collection` start.
    zgc_start_uptime_ms: Option<f64>,
    zgc_start_wall_ms: Option<f64>,
    regions: Regions,
    young: Option<MemChange>,
    old: Option<MemChange>,
    metaspace: Option<MemChange>,
    flags: EventFlags,
}

struct Patterns {
    mem: Regex,
    regions: Regex,
    cpu: Regex,
    /// ZGC heap: `42M(66%)->30M(47%)` (percent of max capacity).
    zgc_mem: Regex,
    /// ZGC metaspace: `Metaspace: 2M used, 3M committed, 1088M reserved`.
    zgc_metaspace: Regex,
}

impl Patterns {
    fn new() -> Self {
        Self {
            mem: mem_change_regex(),
            regions: Regex::new(r"^(Eden|Survivor|Old|Humongous|Archive) regions: (\d+)->(\d+)(?:\((\d+)\))?").unwrap(),
            cpu: Regex::new(r"User=([\d.,]+)s Sys=([\d.,]+)s Real=([\d.,]+)s").unwrap(),
            zgc_mem: Regex::new(r"(\d+(?:\.\d+)?[KMG])\((\d+)%\)->(\d+(?:\.\d+)?[KMG])\((\d+)%\)").unwrap(),
            zgc_metaspace: Regex::new(r"^(\d+(?:\.\d+)?[KMG]) used, (\d+(?:\.\d+)?[KMG]) committed").unwrap(),
        }
    }
}

fn collector_from_using(rest: &str) -> Option<Collector> {
    let rest = rest.trim();
    Some(match rest {
        "G1" => Collector::G1,
        "Parallel" => Collector::Parallel,
        "Serial" => Collector::Serial,
        "Concurrent Mark Sweep" => Collector::Cms,
        "Shenandoah" => Collector::Shenandoah,
        r if r.contains("Z Garbage Collector") || r == "ZGC" || r.starts_with("ZGC") => Collector::Zgc,
        _ => return None,
    })
}

fn is_init_key(key: &str) -> bool {
    const KEYS: [&str; 8] = [
        "Version",
        "CPUs",
        "Memory",
        "Heap Min Capacity",
        "Heap Initial Capacity",
        "Heap Max Capacity",
        "Initial Capacity",
        "Max Capacity",
    ];
    KEYS.contains(&key) || key.eq_ignore_ascii_case("Heap Region Size")
}

/// Cheap check used by format detection: does this line look like unified GC logging?
pub(crate) fn looks_unified(line: &str) -> bool {
    match split_decorators(line) {
        Some(d) => {
            let msg = d.msg;
            (d.tags.is_some_and(|t| t == "gc" || t.starts_with("gc,")) && (msg.starts_with("GC(") || msg.starts_with("Using ")))
                || (d.tags.is_none() && msg.starts_with("GC(") && (msg.contains(" Pause ") || msg.contains(" Concurrent ")))
                || (d.tags.is_none() && msg.starts_with("Using ") && collector_from_using(&msg[6..]).is_some())
        }
        None => false,
    }
}

/// Parse a unified-logging GC log.
pub fn parse(text: &str) -> ParsedGcLog {
    let pats = Patterns::new();
    let mut jvm = JvmInfo::default();
    let mut collector = Collector::Unknown;
    let mut events: Vec<GcEvent> = Vec::new();
    let mut event_regions: Vec<Regions> = Vec::new();
    let mut pending: HashMap<Option<u64>, Pending> = HashMap::new();
    let mut last_event: HashMap<Option<u64>, usize> = HashMap::new();
    let mut unparsed_count = 0;
    let mut unparsed_samples = Vec::new();
    let mut pid = None;
    let mut line_count = 0;
    let (mut first_up, mut last_up, mut first_wall, mut last_wall) = (None, None, None, None);

    let mut note_unparsed = |line_no: usize, line: &str, count: &mut usize| {
        *count += 1;
        if unparsed_samples.len() < MAX_UNPARSED_SAMPLES {
            unparsed_samples.push((line_no, line.to_string()));
        }
    };

    for (idx, raw_line) in text.lines().enumerate() {
        line_count += 1;
        let line_no = idx + 1;
        let line = raw_line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let Some(dec) = split_decorators(line) else {
            note_unparsed(line_no, line, &mut unparsed_count);
            continue;
        };
        if pid.is_none() {
            pid = dec.pid;
        }
        if let Some(u) = dec.uptime_ms {
            first_up.get_or_insert(u);
            last_up = Some(u);
        }
        if let Some(w) = dec.wall_ms {
            first_wall.get_or_insert(w);
            last_wall = Some(w);
        }
        let tags = dec.tags;
        let (gc_id, msg) = split_gc_id(dec.msg);

        // Collector announcement.
        if gc_id.is_none() {
            if let Some(rest) = msg.strip_prefix("Using ") {
                if let Some(c) = collector_from_using(rest) {
                    collector = c;
                    jvm.facts.push(JvmFact { key: "Collector".into(), value: rest.trim().to_string() });
                    continue;
                }
            }
            // gc,init facts and the JDK 11 region size line.
            if let Some((key, value)) = msg.split_once(": ") {
                let key = key.trim();
                let tagged_init = tags.is_some_and(|t| t.split(',').any(|x| x == "init"));
                if tagged_init || is_init_key(key) {
                    let value = value.trim().to_string();
                    match key {
                        "Version" => jvm.version = Some(value.clone()),
                        // ZGC names them `Max Capacity` / `Initial Capacity`.
                        "Heap Max Capacity" | "Max Capacity" => jvm.heap_max_kb = parse_size_kb(&value),
                        "Heap Initial Capacity" | "Initial Capacity" => jvm.heap_initial_kb = parse_size_kb(&value),
                        k if k.eq_ignore_ascii_case("Heap Region Size") => jvm.region_size_kb = parse_size_kb(&value),
                        _ => {}
                    }
                    jvm.facts.push(JvmFact { key: key.to_string(), value });
                    continue;
                }
            }
            // ZGC: `Allocation Stall (main) 1.158ms` — a thread waited for memory.
            if msg.starts_with("Allocation Stall (") {
                if let (Some(d), head) = split_trailing_duration(msg) {
                    events.push(GcEvent {
                        gc_id: None,
                        time_ms: 0.0,
                        uptime_ms: dec.uptime_ms.map(|u| (u - d).max(0.0)),
                        wall_ms: dec.wall_ms.map(|w| w - d),
                        kind: GcKind::Stall,
                        is_pause: false,
                        name: "Allocation Stall".into(),
                        cause: split_paren_groups(head).1.first().map(|t| t.to_string()),
                        duration_ms: d,
                        heap: None,
                        young: None,
                        old: None,
                        humongous: None,
                        metaspace: None,
                        cpu: None,
                        flags: EventFlags { allocation_stall: true, ..Default::default() },
                        file_index: 0,
                        line: line_no,
                    });
                    event_regions.push(Regions::default());
                }
                continue;
            }
            // Other id-less lines (CDS mapping, heap-at-exit, …) are known noise.
            continue;
        }
        // Generational ZGC prefixes per-generation lines with `y: ` / `Y: ` / `o: `.
        let msg = ["y: ", "Y: ", "o: "].iter().find_map(|p| msg.strip_prefix(p)).unwrap_or(msg);

        let is_summary_tag = tags.is_none_or(|t| t == "gc");
        let p = pending.entry(gc_id).or_default();

        // Region / generation / metaspace detail lines.
        if let Some(c) = pats.regions.captures(msg) {
            let before: f64 = c[2].parse().unwrap_or(0.0);
            let after: f64 = c[3].parse().unwrap_or(0.0);
            let cap = c.get(4).and_then(|m| m.as_str().parse().ok());
            match &c[1] {
                "Eden" => p.regions.eden = Some((before, after, cap)),
                "Survivor" => p.regions.survivor = Some((before, after, cap)),
                "Old" => p.regions.old = Some((before, after)),
                "Humongous" => p.regions.humongous = Some((before, after)),
                _ => {}
            }
            if collector == Collector::Unknown {
                collector = Collector::G1;
            }
            continue;
        }
        if let Some((gen, rest)) = msg.split_once(": ") {
            let change = parse_mem_change(&pats.mem, rest);
            let handled = match gen {
                "PSYoungGen" | "DefNew" | "ParNew" => {
                    p.young = change;
                    if collector == Collector::Unknown {
                        collector = match gen {
                            "PSYoungGen" => Collector::Parallel,
                            "DefNew" => Collector::Serial,
                            _ => Collector::Cms,
                        };
                    }
                    true
                }
                "ParOldGen" | "PSOldGen" | "Tenured" | "CMS" => {
                    p.old = change;
                    true
                }
                "Metaspace" => {
                    if change.is_some() {
                        p.metaspace = change;
                    } else if let Some(c) = pats.zgc_metaspace.captures(rest) {
                        // ZGC prints `2M used, 3M committed, 1088M reserved` before its cycle summary.
                        let used = parse_size_kb(&c[1]).unwrap_or(0.0);
                        let committed = parse_size_kb(&c[2]);
                        p.metaspace = Some(MemChange { before_kb: used, after_kb: used, capacity_before_kb: committed, capacity_after_kb: committed });
                    }
                    true
                }
                _ => false,
            };
            if handled {
                continue;
            }
        }
        if let Some(c) = pats.cpu.captures(msg) {
            let cpu = CpuTimes {
                user_ms: parse_decimal(&c[1]).unwrap_or(0.0) * 1000.0,
                sys_ms: parse_decimal(&c[2]).unwrap_or(0.0) * 1000.0,
                real_ms: parse_decimal(&c[3]).unwrap_or(0.0) * 1000.0,
            };
            if let Some(&i) = last_event.get(&gc_id) {
                if events[i].cpu.is_none() {
                    events[i].cpu = Some(cpu);
                }
            }
            continue;
        }
        if msg.starts_with("To-space exhausted") || msg.starts_with("Evacuation Failure") {
            p.flags.evacuation_failure = true;
            continue;
        }

        // ZGC cycles: `Garbage Collection (cause)` (non-generational) or `Minor/Major Collection (cause)`.
        // Non-generational ZGC opens the cycle on `gc,start`; generational ZGC on `gc`.
        let start_tag = tags.is_some_and(|t| t == "gc,start");
        if (is_summary_tag || start_tag) && ["Garbage Collection (", "Minor Collection (", "Major Collection ("].iter().any(|k| msg.starts_with(k)) {
            let (duration, head) = split_trailing_duration(msg);
            let Some(c) = pats.zgc_mem.captures(head) else {
                p.zgc_start_uptime_ms = dec.uptime_ms;
                p.zgc_start_wall_ms = dec.wall_ms;
                continue;
            };
            let size = |i: usize| parse_size_kb(&c[i]);
            let pct = |i: usize| c[i].parse::<f64>().ok().filter(|v| *v > 0.0);
            let (before, after) = (size(1).unwrap_or(0.0), size(3).unwrap_or(0.0));
            let heap = MemChange {
                before_kb: before,
                after_kb: after,
                capacity_before_kb: pct(2).map(|v| before * 100.0 / v),
                capacity_after_kb: pct(4).map(|v| after * 100.0 / v),
            };
            let (base, groups) = split_paren_groups(&head[..c.get(0).map_or(head.len(), |m| m.start())]);
            let start_up = p.zgc_start_uptime_ms.take();
            let start_wall = p.zgc_start_wall_ms.take();
            let duration_ms = duration
                .or_else(|| dec.uptime_ms.zip(start_up).map(|(end, start)| end - start))
                .or_else(|| dec.wall_ms.zip(start_wall).map(|(end, start)| end - start))
                .unwrap_or(0.0)
                .max(0.0);
            collector = Collector::Zgc;
            events.push(GcEvent {
                gc_id,
                time_ms: 0.0,
                uptime_ms: start_up.or(dec.uptime_ms.map(|u| (u - duration_ms).max(0.0))),
                wall_ms: start_wall.or(dec.wall_ms.map(|w| w - duration_ms)),
                kind: GcKind::Concurrent,
                is_pause: false,
                name: base.to_string(),
                cause: groups.first().map(|g| g.to_string()),
                duration_ms,
                heap: Some(heap),
                young: None,
                old: None,
                humongous: None,
                metaspace: p.metaspace.take(),
                cpu: None,
                flags: EventFlags::default(),
                file_index: 0,
                line: line_no,
            });
            event_regions.push(Regions::default());
            last_event.insert(gc_id, events.len() - 1);
            continue;
        }

        let (duration, head) = split_trailing_duration(msg);
        let is_pause = head.starts_with("Pause ");
        let is_cycle = head.starts_with("Concurrent ") && (tags == Some("gc") || head.ends_with("Cycle"));
        if !(is_pause || is_cycle) {
            // Phases, tasks, marking sub-steps, references, …
            continue;
        }
        let Some(duration_ms) = duration else {
            // `gc,start` line (or a concurrent cycle's opening line): remember the start time.
            if is_pause {
                p.start_uptime_ms = dec.uptime_ms;
                p.start_wall_ms = dec.wall_ms;
            } else {
                p.cycle_start_uptime_ms = dec.uptime_ms;
                p.cycle_start_wall_ms = dec.wall_ms;
            }
            continue;
        };
        // ZGC reports its pauses on `gc,phases`.
        let zgc_pause = is_pause && (head.starts_with("Pause Mark") || head.starts_with("Pause Relocate"));
        if !is_summary_tag && !zgc_pause {
            continue;
        }

        let mut head = head;
        let mut heap = None;
        if let Some(m) = pats.mem.find(head) {
            // Pause summaries and Shenandoah's `Concurrent cleanup 8M->3M(32M)`.
            heap = parse_mem_change(&pats.mem, &head[m.start()..]);
            head = head[..m.start()].trim_end();
        }
        let (base, groups) = split_paren_groups(head);
        let mut subtype = None;
        let mut cause = None;
        let mut flags = std::mem::take(&mut p.flags);
        for g in groups {
            if g.starts_with("Evacuation Failure") {
                flags.evacuation_failure = true;
            } else if subtype.is_none() && cause.is_none() && G1_YOUNG_SUBTYPES.contains(&g) {
                subtype = Some(g);
            } else if cause.is_none() {
                cause = Some(g.to_string());
            }
        }
        let mut name = match subtype {
            Some(s) => format!("{base} ({s})"),
            None => base.to_string(),
        };
        if collector == Collector::Shenandoah {
            // `(unload classes)`, `(Outside of Cycle)` qualify the phase; they are not causes.
            name = head.to_string();
            cause = None;
            flags.degenerated |= base.contains("Degenerated");
        }
        if collector == Collector::Unknown
            && (subtype.is_some() || cause.as_deref().is_some_and(|c| c.starts_with("G1 ")))
        {
            collector = Collector::G1;
        }
        let (logged_uptime, logged_wall) = if is_pause {
            (p.start_uptime_ms.take(), p.start_wall_ms.take())
        } else {
            (p.cycle_start_uptime_ms.take(), p.cycle_start_wall_ms.take())
        };
        let start_uptime = logged_uptime.or(dec.uptime_ms.map(|u| (u - duration_ms).max(0.0)));
        let start_wall = logged_wall.or(dec.wall_ms.map(|w| w - duration_ms));
        let kind = if is_pause { kind_for(base, subtype) } else { GcKind::Concurrent };
        let regions = std::mem::take(&mut p.regions);
        events.push(GcEvent {
            gc_id,
            time_ms: 0.0,
            uptime_ms: start_uptime,
            wall_ms: start_wall,
            kind,
            is_pause,
            name,
            cause,
            duration_ms,
            heap,
            young: p.young.take(),
            old: p.old.take(),
            humongous: None,
            metaspace: p.metaspace.take(),
            cpu: None,
            flags,
            file_index: 0,
            line: line_no,
        });
        event_regions.push(regions);
        last_event.insert(gc_id, events.len() - 1);
    }

    finalize(&mut events, &event_regions, jvm.region_size_kb);
    if collector == Collector::Zgc {
        // ZGC percentages are of the max capacity; prefer the exact configured value.
        if let Some(max) = jvm.heap_max_kb {
            for h in events.iter_mut().filter_map(|e| e.heap.as_mut()) {
                h.capacity_before_kb = Some(max);
                h.capacity_after_kb = Some(max);
            }
        }
    }
    let missing_timestamps = !events.is_empty() && events.iter().all(|e| e.uptime_ms.is_none() && e.wall_ms.is_none());

    ParsedGcLog {
        format: GcLogFormat::Unified,
        collector,
        jvm,
        events,
        line_count,
        unparsed_count,
        unparsed_samples,
        pid,
        missing_timestamps,
        first_line_uptime_ms: first_up,
        last_line_uptime_ms: last_up,
        first_line_wall_ms: first_wall,
        last_line_wall_ms: last_wall,
        rotation_created: None,
        rotation_saved_as: None,
    }
}

/// Convert G1 region counts to KiB, assign the analysis time axis and sort.
fn finalize(events: &mut [GcEvent], regions: &[Regions], region_size_kb: Option<f64>) {
    if let Some(rs) = region_size_kb {
        for (e, r) in events.iter_mut().zip(regions) {
            if r.is_empty() {
                continue;
            }
            if e.young.is_none() {
                if let (Some(eden), Some(surv)) = (r.eden, r.survivor) {
                    let cap = match (eden.2, surv.2) {
                        (Some(a), Some(b)) => Some((a + b) * rs),
                        _ => None,
                    };
                    e.young = Some(MemChange {
                        before_kb: (eden.0 + surv.0) * rs,
                        after_kb: (eden.1 + surv.1) * rs,
                        capacity_before_kb: None,
                        capacity_after_kb: cap,
                    });
                }
            }
            if e.old.is_none() {
                if let Some((b, a)) = r.old {
                    e.old = Some(MemChange { before_kb: b * rs, after_kb: a * rs, capacity_before_kb: None, capacity_after_kb: None });
                }
            }
            if let Some((b, a)) = r.humongous {
                e.humongous = Some(MemChange { before_kb: b * rs, after_kb: a * rs, capacity_before_kb: None, capacity_after_kb: None });
            }
        }
    }
    assign_time_axis(events);
}

/// Pick the analysis time axis (uptime, else wall clock relative to the first event, else order) and sort.
pub(crate) fn assign_time_axis(events: &mut [GcEvent]) {
    let has_uptime = events.iter().any(|e| e.uptime_ms.is_some());
    let first_wall = events.iter().filter_map(|e| e.wall_ms).fold(f64::INFINITY, f64::min);
    for (i, e) in events.iter_mut().enumerate() {
        e.time_ms = if has_uptime {
            e.uptime_ms.unwrap_or(0.0)
        } else if first_wall.is_finite() {
            e.wall_ms.map(|w| w - first_wall).unwrap_or(0.0)
        } else {
            i as f64
        };
    }
    // Total order (line breaks ties), so the smaller unstable sort is enough.
    events.sort_unstable_by(|a, b| a.time_ms.total_cmp(&b.time_ms).then(a.gc_id.cmp(&b.gc_id)).then(a.line.cmp(&b.line)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorators_recognized_by_content() {
        let d = split_decorators("[2026-11-13T16:22:15.343+0800][4.642s][info][gc             ] GC(29) Concurrent Mark Cycle").unwrap();
        assert_eq!(d.uptime_ms, Some(4642.0));
        assert!(d.wall_ms.is_some());
        assert_eq!(d.level, Some("info"));
        assert_eq!(d.tags, Some("gc"));
        assert_eq!(d.msg, "GC(29) Concurrent Mark Cycle");

        let d = split_decorators("[2026-11-13T08:22:26.267+0000][45114][45122][info][gc,start    ] GC(0) Pause Young").unwrap();
        assert_eq!(d.pid, Some(45114));
        assert_eq!(d.tags, Some("gc,start"));

        let d = split_decorators("[559ms] GC(0) Pause Young (Normal) (G1 Evacuation Pause) 34M->1M(64M) 0.384ms").unwrap();
        assert_eq!(d.uptime_ms, Some(559.0));
        assert_eq!(d.tags, None);

        let d = split_decorators("[1699999999999ms][info][gc] Using G1").unwrap();
        assert_eq!(d.wall_ms, Some(1_699_999_999_999.0));
        assert_eq!(d.uptime_ms, None);

        let d = split_decorators("[0,017s][info][gc] Using G1").unwrap();
        assert_eq!(d.uptime_ms, Some(17.0));

        // JDK 8 lines and application output are not unified logging.
        assert!(split_decorators("[PSYoungGen: 1132814K->20723K(2446848K)] 1132814K->20739K(8039424K), 0.019 secs]").is_none());
        assert!(split_decorators("2026-08-18T16:23:24.623+0800: 21.953: [GC (Metadata GC Threshold)").is_none());
        assert!(split_decorators("hello world").is_none());
    }

    #[test]
    fn paren_groups_handle_nested_system_gc() {
        let (base, groups) = split_paren_groups("Pause Full (System.gc())");
        assert_eq!(base, "Pause Full");
        assert_eq!(groups, vec!["System.gc()"]);
        let (base, groups) = split_paren_groups("Pause Young (Prepare Mixed) (G1 Evacuation Pause) (Evacuation Failure)");
        assert_eq!(base, "Pause Young");
        assert_eq!(groups, vec!["Prepare Mixed", "G1 Evacuation Pause", "Evacuation Failure"]);
    }

    #[test]
    fn trailing_duration_units() {
        assert_eq!(split_trailing_duration("Pause Remark 20M->20M(37M) 0.106ms"), (Some(0.106), "Pause Remark 20M->20M(37M)"));
        assert_eq!(split_trailing_duration("Concurrent Mark Cycle 1.5s"), (Some(1500.0), "Concurrent Mark Cycle"));
        assert_eq!(split_trailing_duration("Pause Remark"), (None, "Pause Remark"));
    }
}
