//! Multi-file fingerprinting, grouping and stitching (feat-073).
//!
//! Each file is parsed on its own to build a [`FileFingerprint`]. Files are
//! grouped into JVMs (auto / merge / split), ordered by their line timestamps,
//! then each group's texts are concatenated and parsed once more so events
//! split across rotation boundaries are reassembled.
//!
//! Same-JVM evidence, strongest first:
//! 1. JVM start instant (`wall clock − uptime`) equal within [`JVM_START_TOLERANCE_MS`];
//! 2. equal `pid` decorator with non-overlapping line timestamps;
//! 3. same collector, non-overlapping uptime and contiguous `GC(n)` ids.
//!
//! Any contradiction (different collector, dialect or pid) means different JVMs.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use super::model::{Collector, GcLogFormat, ParsedGcLog};
use super::parse_log;
use super::units::small_sort_by;

/// Two JVM start instants within this distance are the same JVM.
pub const JVM_START_TOLERANCE_MS: f64 = 2000.0;

/// One input file.
#[derive(Debug, Clone, Copy)]
pub struct GcInput<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

/// How files are grouped into JVMs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupingMode {
    /// Rotated segments of one JVM are stitched; different JVMs stay apart.
    #[default]
    Auto,
    /// Treat every file as a segment of the same JVM.
    Merge,
    /// Every file is its own group.
    Split,
}

impl GroupingMode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "auto" => Some(Self::Auto),
            "merge" => Some(Self::Merge),
            "split" => Some(Self::Split),
            _ => None,
        }
    }
}

/// Identity and range facts of one file, used for grouping and diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileFingerprint {
    pub name: String,
    pub format: GcLogFormat,
    pub collector: Collector,
    pub version: Option<String>,
    pub pid: Option<u64>,
    pub first_gc_id: Option<u64>,
    pub last_gc_id: Option<u64>,
    pub first_uptime_ms: Option<f64>,
    pub last_uptime_ms: Option<f64>,
    pub first_wall_ms: Option<f64>,
    pub last_wall_ms: Option<f64>,
    /// `wall − uptime` of the first line carrying both.
    pub jvm_start_ms: Option<f64>,
    /// JDK 8 rotation markers (`GC log file created …` / `… Saved as …`).
    pub rotation_created: Option<String>,
    pub rotation_saved_as: Option<String>,
    pub event_count: usize,
    pub line_count: usize,
    pub unparsed_count: usize,
    pub unparsed_samples: Vec<(usize, String)>,
    #[serde(skip)]
    pub digest: u64,
}

/// Kind of a stitching diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteKind {
    /// `GC(n)` ids jump between consecutive segments (a segment is missing).
    Gap,
    /// Consecutive segments overlap in time.
    Overlap,
    /// The same file content was imported twice; the copy was ignored.
    Duplicate,
    /// The earliest segment does not start at JVM startup (`GC(0)` missing).
    MissingStart,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupNote {
    pub kind: NoteKind,
    /// Input index of the file the note is about.
    pub file_index: usize,
    /// Machine-readable detail (`missing=12`, `overlap_ms=340`, `duplicate_of=0`, `first_gc_id=89`).
    pub detail: String,
}

/// One JVM: an ordered set of segments and their stitched parse.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcGroup {
    /// Name of the earliest segment.
    pub name: String,
    /// Input indices in time order (duplicates excluded).
    pub files: Vec<usize>,
    pub log: ParsedGcLog,
    pub notes: Vec<GroupNote>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stitched {
    pub mode: GroupingMode,
    /// Per input file, in input order.
    pub files: Vec<FileFingerprint>,
    pub groups: Vec<GcGroup>,
}

fn digest(text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

fn fingerprint(name: &str, text: &str, log: &ParsedGcLog) -> FileFingerprint {
    let ids = log.events.iter().filter_map(|e| e.gc_id);
    let jvm_start_ms = match (log.first_line_wall_ms, log.first_line_uptime_ms) {
        (Some(w), Some(u)) => Some(w - u),
        _ => None,
    };
    FileFingerprint {
        name: name.to_string(),
        format: log.format,
        collector: log.collector,
        version: log.jvm.version.clone(),
        pid: log.pid,
        first_gc_id: ids.clone().min(),
        last_gc_id: ids.max(),
        first_uptime_ms: log.first_line_uptime_ms,
        last_uptime_ms: log.last_line_uptime_ms,
        first_wall_ms: log.first_line_wall_ms,
        last_wall_ms: log.last_line_wall_ms,
        jvm_start_ms,
        rotation_created: log.rotation_created.clone(),
        rotation_saved_as: log.rotation_saved_as.clone(),
        event_count: log.events.len(),
        line_count: log.line_count,
        unparsed_count: log.unparsed_count,
        unparsed_samples: log.unparsed_samples.clone(),
        digest: digest(text),
    }
}

/// Sort key: line uptime, then wall clock, then first GC id.
fn order_key(f: &FileFingerprint) -> (f64, f64, u64) {
    (
        f.first_uptime_ms.unwrap_or(f64::INFINITY),
        f.first_wall_ms.unwrap_or(f64::INFINITY),
        f.first_gc_id.unwrap_or(u64::MAX),
    )
}

fn cmp_order(a: &FileFingerprint, b: &FileFingerprint) -> std::cmp::Ordering {
    let (ka, kb) = (order_key(a), order_key(b));
    ka.0.total_cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then(ka.2.cmp(&kb.2))
}

/// `/logs/gc.log.3` and `/logs/gc.log.3.current` → `/logs/gc.log`.
fn rotation_base(path: &str) -> &str {
    let p = path.trim().strip_suffix(".current").unwrap_or(path.trim());
    match p.rfind('.') {
        Some(i) if p[i + 1..].chars().all(|c| c.is_ascii_digit()) && i + 1 < p.len() => &p[..i],
        _ => p,
    }
}

/// Do `a` and `b` belong to the same JVM? (auto mode)
pub fn same_jvm(a: &FileFingerprint, b: &FileFingerprint) -> bool {
    if a.format == GcLogFormat::Unknown || b.format == GcLogFormat::Unknown || a.format != b.format {
        return false;
    }
    if a.collector != Collector::Unknown && b.collector != Collector::Unknown && a.collector != b.collector {
        return false;
    }
    if let (Some(p), Some(q)) = (a.pid, b.pid) {
        if p != q {
            return false;
        }
    }
    if let (Some(s), Some(t)) = (a.jvm_start_ms, b.jvm_start_ms) {
        return (s - t).abs() <= JVM_START_TOLERANCE_MS;
    }
    let (x, y) = if cmp_order(a, b).is_le() { (a, b) } else { (b, a) };
    let non_overlapping = match (x.last_uptime_ms, y.first_uptime_ms) {
        (Some(end), Some(start)) => start >= end,
        _ => false,
    };
    if !non_overlapping {
        return false;
    }
    if a.pid.is_some() && a.pid == b.pid {
        return true;
    }
    // JDK 8 rotation: the earlier segment was "Saved as" and the later one "created" for the same log file.
    if let (Some(saved), Some(created)) = (&x.rotation_saved_as, &y.rotation_created) {
        if rotation_base(saved) == rotation_base(created) {
            return true;
        }
    }
    match (x.last_gc_id, y.first_gc_id) {
        // A boundary-split event shares its id across both segments.
        (Some(last), Some(first)) => first == last || first == last + 1,
        _ => false,
    }
}

/// Minimal union-find over input indices.
fn find(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut j = i;
    while parent[j] != r {
        let next = parent[j];
        parent[j] = r;
        j = next;
    }
    r
}

fn partition(files: &[FileFingerprint], usable: &[usize], mode: GroupingMode) -> Vec<Vec<usize>> {
    match mode {
        GroupingMode::Split => usable.iter().map(|&i| vec![i]).collect(),
        GroupingMode::Merge => {
            if usable.is_empty() {
                Vec::new()
            } else {
                vec![usable.to_vec()]
            }
        }
        GroupingMode::Auto => {
            let mut parent: Vec<usize> = (0..files.len()).collect();
            for (n, &i) in usable.iter().enumerate() {
                for &j in &usable[n + 1..] {
                    if same_jvm(&files[i], &files[j]) {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                        parent[rj.max(ri)] = ri.min(rj);
                    }
                }
            }
            let mut by_root: Vec<(usize, Vec<usize>)> = Vec::new();
            for &i in usable {
                let r = find(&mut parent, i);
                match by_root.iter_mut().find(|(root, _)| *root == r) {
                    Some((_, members)) => members.push(i),
                    None => by_root.push((r, vec![i])),
                }
            }
            by_root.into_iter().map(|(_, m)| m).collect()
        }
    }
}

fn notes_for(files: &[FileFingerprint], ordered: &[usize]) -> Vec<GroupNote> {
    let mut notes = Vec::new();
    if let Some(&first) = ordered.first() {
        let f = &files[first];
        if f.format == GcLogFormat::Unified && f.first_gc_id.is_some_and(|id| id > 0) {
            notes.push(GroupNote {
                kind: NoteKind::MissingStart,
                file_index: first,
                detail: format!("first_gc_id={}", f.first_gc_id.unwrap()),
            });
        }
    }
    for w in ordered.windows(2) {
        let (x, y) = (&files[w[0]], &files[w[1]]);
        if let (Some(last), Some(first)) = (x.last_gc_id, y.first_gc_id) {
            if first > last + 1 {
                notes.push(GroupNote { kind: NoteKind::Gap, file_index: w[1], detail: format!("missing={}", first - last - 1) });
            }
        }
        let overlap = match (x.last_uptime_ms, y.first_uptime_ms) {
            (Some(end), Some(start)) if start < end => Some(end - start),
            _ => match (x.last_wall_ms, y.first_wall_ms) {
                (Some(end), Some(start)) if start < end => Some(end - start),
                _ => None,
            },
        };
        if let Some(ms) = overlap {
            notes.push(GroupNote { kind: NoteKind::Overlap, file_index: w[1], detail: format!("overlap_ms={ms:.0}") });
        }
    }
    notes
}

/// Concatenate the group's texts in order, parse once, map lines back to files and drop overlap duplicates.
fn stitch_group(inputs: &[GcInput], ordered: &[usize]) -> ParsedGcLog {
    let mut text = String::new();
    // (input index, first global line, line count)
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    let mut next_line = 1;
    for &i in ordered {
        let t = inputs[i].text;
        let lines = t.lines().count();
        spans.push((i, next_line, lines));
        next_line += lines;
        text.push_str(t);
        if !t.is_empty() && !t.ends_with('\n') {
            text.push('\n');
        }
    }
    let mut log = parse_log(&text);
    for e in &mut log.events {
        if let Some(&(file, start, _)) = spans.iter().rev().find(|(_, start, _)| *start <= e.line) {
            e.file_index = file;
            e.line = e.line - start + 1;
        }
    }
    // Within one JVM a `GC(n)` id + name identifies an event; the start time may differ
    // when an overlapping copy lacks the `gc,start` line, so it is only used without ids.
    let mut seen = HashSet::new();
    log.events.retain(|e| {
        let t = match e.gc_id {
            Some(_) => e.duration_ms.to_bits() as i64,
            None => e.uptime_ms.or(e.wall_ms).map_or(0, |v| (v * 1000.0).round() as i64),
        };
        seen.insert((e.gc_id, e.name.clone(), e.cause.clone(), t))
    });
    for (line, _) in &mut log.unparsed_samples {
        if let Some(&(_, start, _)) = spans.iter().rev().find(|(_, start, _)| *start <= *line) {
            *line = *line - start + 1;
        }
    }
    log
}

/// Fingerprint, group and stitch `inputs`.
pub fn stitch(inputs: &[GcInput], mode: GroupingMode) -> Stitched {
    let parsed: Vec<ParsedGcLog> = inputs.iter().map(|i| parse_log(i.text)).collect();
    let files: Vec<FileFingerprint> =
        inputs.iter().zip(&parsed).map(|(i, log)| fingerprint(i.name, i.text, log)).collect();

    // Exact duplicates are ignored (noted on the group that keeps the original).
    let mut first_by_digest: HashMap<u64, usize> = HashMap::new();
    let mut duplicates: Vec<(usize, usize)> = Vec::new();
    let mut usable = Vec::new();
    for (i, f) in files.iter().enumerate() {
        if f.format == GcLogFormat::Unknown {
            continue;
        }
        match first_by_digest.get(&f.digest) {
            Some(&orig) => duplicates.push((i, orig)),
            None => {
                first_by_digest.insert(f.digest, i);
                usable.push(i);
            }
        }
    }

    let mut parsed: Vec<Option<ParsedGcLog>> = parsed.into_iter().map(Some).collect();
    let mut groups: Vec<GcGroup> = partition(&files, &usable, mode)
        .into_iter()
        .map(|mut members| {
            small_sort_by(&mut members, |&a, &b| cmp_order(&files[a], &files[b]).then(a.cmp(&b)));
            let log = if members.len() == 1 {
                let mut log = parsed[members[0]].take().expect("each file is in one group");
                for e in &mut log.events {
                    e.file_index = members[0];
                }
                log
            } else {
                stitch_group(inputs, &members)
            };
            let mut notes = notes_for(&files, &members);
            for &(dup, orig) in &duplicates {
                if members.contains(&orig) {
                    notes.push(GroupNote { kind: NoteKind::Duplicate, file_index: dup, detail: format!("duplicate_of={orig}") });
                }
            }
            GcGroup { name: files[members[0]].name.clone(), files: members, log, notes }
        })
        .collect();
    // Stable presentation: by earliest wall clock / uptime, then input order.
    small_sort_by(&mut groups, |a, b| cmp_order(&files[a.files[0]], &files[b.files[0]]).then(a.files[0].cmp(&b.files[0])));
    Stitched { mode, files, groups }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::GcKind;

    const ROT_0: &str = include_str!("../../tests/fixtures/gc/g1-rotated-jdk21.log.0");
    const ROT_1: &str = include_str!("../../tests/fixtures/gc/g1-rotated-jdk21.log.1");
    const ROT_2: &str = include_str!("../../tests/fixtures/gc/g1-rotated-jdk21.log.2");
    const ROT_CUR: &str = include_str!("../../tests/fixtures/gc/g1-rotated-jdk21.log");
    const WALL_0: &str = include_str!("../../tests/fixtures/gc/g1-rotated-wall-jdk21.log.0");
    const WALL_CUR: &str = include_str!("../../tests/fixtures/gc/g1-rotated-wall-jdk21.log");
    const PARALLEL: &str = include_str!("../../tests/fixtures/gc/parallel-jdk21.log");
    const G1_STRESS: &str = include_str!("../../tests/fixtures/gc/g1-stress-jdk21.log");

    fn inp<'a>(name: &'a str, text: &'a str) -> GcInput<'a> {
        GcInput { name, text }
    }

    #[test]
    fn stitch_rotated_segments_in_any_order() {
        // Shuffled input order; names do not reflect time order either.
        let inputs = [inp("gc.log", ROT_CUR), inp("gc.log.2", ROT_2), inp("gc.log.0", ROT_0), inp("gc.log.1", ROT_1)];
        let s = stitch(&inputs, GroupingMode::Auto);
        assert_eq!(s.groups.len(), 1);
        let g = &s.groups[0];
        assert_eq!(g.files, vec![2, 3, 1, 0]);
        assert_eq!(g.name, "gc.log.0");
        assert!(g.notes.is_empty(), "{:?}", g.notes);
        assert_eq!(g.log.collector, Collector::G1);

        // Same events as parsing the concatenation in time order (the boundary-split events are whole).
        let whole = parse_log(&[ROT_0, ROT_1, ROT_2, ROT_CUR].concat());
        assert_eq!(g.log.events.len(), whole.events.len());
        let ids: Vec<_> = g.log.events.iter().filter(|e| e.is_pause).filter_map(|e| e.gc_id).collect();
        assert_eq!(ids.first(), Some(&0));
        assert_eq!(ids.last(), Some(&103)); // GC(104) is a concurrent undo cycle
        assert_eq!(g.log.events.last().unwrap().gc_id, Some(104));
        // GC(57) straddles .1/.2: heap lines in .1, summary in .2 → reassembled with its old-gen data.
        let e57 = g.log.events.iter().find(|e| e.gc_id == Some(57) && e.is_pause).unwrap();
        assert_eq!(e57.file_index, 1, "summary line is in gc.log.2 (input index 1)");
        assert!(e57.metaspace.is_some() && e57.cpu.is_some());
        // Line numbers are local to the file.
        assert!(g.log.events.iter().all(|e| e.line >= 1 && e.line <= inputs[e.file_index].text.lines().count()));
    }

    #[test]
    fn stitch_detects_jvm_start_identity_and_separates_jvms() {
        let inputs = [inp("a.log", WALL_CUR), inp("b.log", PARALLEL), inp("c.log", WALL_0), inp("d.log", G1_STRESS)];
        let s = stitch(&inputs, GroupingMode::Auto);
        assert_eq!(s.groups.len(), 3);
        let wall = s.groups.iter().find(|g| g.files.contains(&0)).unwrap();
        assert_eq!(wall.files, vec![2, 0]);
        assert!(s.files[0].jvm_start_ms.is_some() && s.files[0].pid.is_some());
        assert!((s.files[0].jvm_start_ms.unwrap() - s.files[2].jvm_start_ms.unwrap()).abs() < JVM_START_TOLERANCE_MS);
    }

    #[test]
    fn stitch_gap_missing_start_and_duplicate_notes() {
        // Missing middle segment (.1): ids jump. Without pid/wall the strict rule keeps them apart.
        let s = stitch(&[inp("0", ROT_0), inp("2", ROT_2)], GroupingMode::Auto);
        assert_eq!(s.groups.len(), 2);
        // Forced merge reports the gap.
        let s = stitch(&[inp("0", ROT_0), inp("2", ROT_2)], GroupingMode::Merge);
        assert_eq!(s.groups.len(), 1);
        assert!(s.groups[0].notes.iter().any(|n| n.kind == NoteKind::Gap && n.file_index == 1));

        // Missing first segment: no header, collector inferred, flagged as not starting at GC(0).
        let s = stitch(&[inp("1", ROT_1), inp("2", ROT_2)], GroupingMode::Auto);
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].log.collector, Collector::G1);
        assert_eq!(s.groups[0].notes[0].kind, NoteKind::MissingStart);

        // The same file twice: the copy is ignored and noted.
        let s = stitch(&[inp("x", ROT_0), inp("x copy", ROT_0)], GroupingMode::Auto);
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].files, vec![0]);
        assert!(s.groups[0].notes.iter().any(|n| n.kind == NoteKind::Duplicate && n.file_index == 1));
    }

    #[test]
    fn stitch_overlap_is_deduplicated() {
        // Segment .1 plus a copy of its second half: overlapping content must not double-count events.
        let half: String = ROT_1.lines().skip(160).map(|l| format!("{l}\n")).collect();
        let s = stitch(&[inp("1", ROT_1), inp("1-tail", &half)], GroupingMode::Merge);
        let g = &s.groups[0];
        assert!(g.notes.iter().any(|n| n.kind == NoteKind::Overlap));
        let alone = parse_log(ROT_1);
        assert_eq!(
            g.log.events.iter().filter(|e| e.kind != GcKind::Concurrent).count(),
            alone.events.iter().filter(|e| e.kind != GcKind::Concurrent).count()
        );
    }

    #[test]
    fn stitch_split_and_merge_modes() {
        let inputs = [inp("0", ROT_0), inp("1", ROT_1), inp("p", PARALLEL)];
        assert_eq!(stitch(&inputs, GroupingMode::Split).groups.len(), 3);
        assert_eq!(stitch(&inputs, GroupingMode::Merge).groups.len(), 1);
        assert_eq!(stitch(&inputs, GroupingMode::Auto).groups.len(), 2);
        assert_eq!(GroupingMode::parse("merge"), Some(GroupingMode::Merge));
        assert_eq!(GroupingMode::parse("nope"), None);
    }

    #[test]
    fn stitch_jdk8_rotation_markers() {
        let r0 = include_str!("../../tests/fixtures/gc/jdk8-parallel-rotated.log.0");
        let r1 = include_str!("../../tests/fixtures/gc/jdk8-parallel-rotated.log.1");
        let s = stitch(&[inp("gc.log.1", r1), inp("gc.log.0", r0)], GroupingMode::Auto);
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].files, vec![1, 0]);
        assert!(s.files[1].rotation_saved_as.as_deref().unwrap().ends_with("jdk8-parallel-rotated.log.0"));
        assert!(s.files[0].rotation_created.as_deref().unwrap().ends_with("jdk8-parallel-rotated.log.1"));
        // Without date stamps there is no JVM start instant: the rotation markers alone tie the segments.
        let re = regex::Regex::new(r"(?m)\d{4}-\d\d-\d\dT[\d:.]+[+-]\d{4}: ").unwrap();
        let (b0, b1) = (re.replace_all(r0, "").into_owned(), re.replace_all(r1, "").into_owned());
        let s = stitch(&[inp("a", &b0), inp("b", &b1)], GroupingMode::Auto);
        assert!(s.files.iter().all(|f| f.jvm_start_ms.is_none()));
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].log.events.len(), 58);
        assert_eq!(rotation_base("/x/gc.log.12.current"), "/x/gc.log");
        assert_eq!(rotation_base("/x/gc.log"), "/x/gc.log");
    }

    #[test]
    fn stitch_ignores_non_gc_files() {
        let s = stitch(&[inp("dump", include_str!("../../tests/fixtures/deadlock_real_jstack.txt")), inp("gc", ROT_0)], GroupingMode::Auto);
        assert_eq!(s.files[0].format, GcLogFormat::Unknown);
        assert_eq!(s.groups.len(), 1);
        assert_eq!(s.groups[0].files, vec![1]);
    }
}
