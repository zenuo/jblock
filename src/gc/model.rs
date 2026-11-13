//! GC log data model shared by the parsers, stats, findings, CLI and WASM.
//!
//! Keep `web/src/gcTypes.ts` in sync with the serialized shapes below.

use serde::{Deserialize, Serialize};

/// Which GC log dialect a file was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GcLogFormat {
    /// JDK 9+ unified logging (`-Xlog:gc*`).
    Unified,
    /// JDK 8 and earlier `-XX:+PrintGCDetails` output.
    Legacy,
    /// Not recognized as a GC log.
    Unknown,
}

/// Garbage collector that produced the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Collector {
    G1,
    Parallel,
    Serial,
    Cms,
    Zgc,
    Shenandoah,
    Unknown,
}

impl Collector {
    /// Human-readable collector name.
    pub fn label(self) -> &'static str {
        match self {
            Collector::G1 => "G1",
            Collector::Parallel => "Parallel",
            Collector::Serial => "Serial",
            Collector::Cms => "CMS",
            Collector::Zgc => "ZGC",
            Collector::Shenandoah => "Shenandoah",
            Collector::Unknown => "Unknown",
        }
    }
}

/// Coarse event category used for coloring, counting and rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GcKind {
    /// Young-only collection (G1 Normal / Concurrent Start / Prepare Mixed, Parallel/Serial young).
    Young,
    /// G1 mixed collection (young + some old regions).
    Mixed,
    /// Full, stop-the-world collection of the whole heap.
    Full,
    /// Concurrent-cycle remark pause (G1 Remark, CMS Final Remark).
    Remark,
    /// G1 cleanup pause.
    Cleanup,
    /// CMS initial mark pause.
    InitialMark,
    /// Any other stop-the-world pause (ZGC / Shenandoah sub-pauses, …).
    OtherPause,
    /// Concurrent work that does not stop application threads.
    Concurrent,
    /// An application thread stalled waiting for memory (ZGC Allocation Stall); not a GC pause.
    Stall,
}

impl GcKind {
    /// All kinds in code order (the `kind` series column stores the index).
    pub const ALL: [GcKind; 9] = [
        GcKind::Young,
        GcKind::Mixed,
        GcKind::Full,
        GcKind::Remark,
        GcKind::Cleanup,
        GcKind::InitialMark,
        GcKind::OtherPause,
        GcKind::Concurrent,
        GcKind::Stall,
    ];

    pub fn code(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// A before → after memory change in KiB, with optional capacities.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MemChange {
    pub before_kb: f64,
    pub after_kb: f64,
    /// Committed capacity before the collection, when the log prints it.
    pub capacity_before_kb: Option<f64>,
    /// Committed capacity after the collection, when the log prints it.
    pub capacity_after_kb: Option<f64>,
}

/// CPU time reported for one collection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CpuTimes {
    pub user_ms: f64,
    pub sys_ms: f64,
    pub real_ms: f64,
}

/// Failure / degradation markers attached to an event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct EventFlags {
    /// G1 evacuation failure / to-space exhausted.
    pub evacuation_failure: bool,
    /// CMS / ParNew promotion failed.
    pub promotion_failed: bool,
    /// CMS concurrent mode failure.
    pub concurrent_mode_failure: bool,
    /// ZGC allocation stall.
    pub allocation_stall: bool,
    /// Shenandoah degenerated GC.
    pub degenerated: bool,
}

/// One garbage collection event (a pause or a concurrent cycle).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GcEvent {
    /// Unified-logging `GC(n)` id, when present.
    pub gc_id: Option<u64>,
    /// Start time on the analysis time axis in ms (uptime, else wall clock relative to the first event).
    pub time_ms: f64,
    /// JVM uptime at the start of the event in ms, when logged.
    pub uptime_ms: Option<f64>,
    /// Wall-clock start time as epoch ms, when logged.
    pub wall_ms: Option<f64>,
    pub kind: GcKind,
    pub is_pause: bool,
    /// Event name without cause, e.g. `Pause Young (Normal)` or `Concurrent Mark Cycle`.
    pub name: String,
    /// GC cause, e.g. `G1 Evacuation Pause`, `Allocation Failure`, `System.gc()`.
    pub cause: Option<String>,
    pub duration_ms: f64,
    pub heap: Option<MemChange>,
    pub young: Option<MemChange>,
    pub old: Option<MemChange>,
    pub humongous: Option<MemChange>,
    pub metaspace: Option<MemChange>,
    pub cpu: Option<CpuTimes>,
    pub flags: EventFlags,
    /// Index of the input file the summary line came from (set when stitching files).
    pub file_index: usize,
    /// 1-based line number of the event's summary line in its file.
    pub line: usize,
}

/// One `key: value` fact about the JVM (version, CPUs, heap sizes, flags, …).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JvmFact {
    pub key: String,
    pub value: String,
}

/// JVM facts extracted from the log header / `gc,init` lines.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct JvmInfo {
    pub version: Option<String>,
    pub heap_max_kb: Option<f64>,
    pub heap_initial_kb: Option<f64>,
    pub region_size_kb: Option<f64>,
    /// `MaxGCPauseMillis` when it can be read from the log.
    pub pause_target_ms: Option<f64>,
    /// JVM command line flags (JDK 8 `CommandLine flags:` header).
    pub command_line: Option<String>,
    /// All raw facts in log order.
    pub facts: Vec<JvmFact>,
}

/// Result of parsing one GC log file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedGcLog {
    pub format: GcLogFormat,
    pub collector: Collector,
    pub jvm: JvmInfo,
    /// Events sorted by start time.
    pub events: Vec<GcEvent>,
    /// Total number of lines in the input.
    pub line_count: usize,
    /// Lines that looked like GC output but could not be understood, plus non-GC noise.
    pub unparsed_count: usize,
    /// First few unparsed lines (`line number`, text) for diagnostics.
    pub unparsed_samples: Vec<(usize, String)>,
    /// `pid` decorator value, when logged (used to group rotated files).
    pub pid: Option<u64>,
    /// True when no event carried an uptime or wall-clock timestamp.
    pub missing_timestamps: bool,
    /// Timestamps of the first / last timestamped line (any line, not just events).
    /// Line timestamps are monotonic across rotated segments, unlike event starts.
    pub first_line_uptime_ms: Option<f64>,
    pub last_line_uptime_ms: Option<f64>,
    pub first_line_wall_ms: Option<f64>,
    pub last_line_wall_ms: Option<f64>,
    /// JDK 8 rotation: `GC log file created <path>` at the top of a rotated file.
    pub rotation_created: Option<String>,
    /// JDK 8 rotation: `GC log file has reached the maximum size. Saved as <path>` at the end.
    pub rotation_saved_as: Option<String>,
}

/// Maximum number of unparsed lines kept as samples.
pub const MAX_UNPARSED_SAMPLES: usize = 20;
