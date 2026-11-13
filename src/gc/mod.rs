//! Pure-Rust JVM GC log parsing and analysis (feat-068+).
//!
//! Like [`crate::parser`] for thread dumps, this module has no `wasm-bindgen`
//! dependency so it is unit-testable with `cargo test`. Design:
//! `docs/design/gc-log-analysis.md`.

pub mod findings;
pub mod legacy;
pub mod model;
pub mod report;
pub mod stats;
pub mod stitch;
pub mod unified;
pub mod units;

#[cfg(test)]
mod tests;

pub use model::{
    Collector, CpuTimes, EventFlags, GcEvent, GcKind, GcLogFormat, JvmFact, JvmInfo, MemChange, ParsedGcLog,
};

/// Number of leading lines inspected when sniffing the log dialect.
const SNIFF_LINES: usize = 400;

/// Detect whether `text` is a unified (JDK 9+) or legacy (JDK 8) GC log.
pub fn detect_format(text: &str) -> GcLogFormat {
    let mut unified = 0;
    let mut legacy = 0;
    for line in text.lines().take(SNIFF_LINES) {
        if unified::looks_unified(line) {
            unified += 1;
        } else if looks_legacy(line) {
            legacy += 1;
        }
    }
    if unified == 0 && legacy == 0 {
        GcLogFormat::Unknown
    } else if unified >= legacy {
        GcLogFormat::Unified
    } else {
        GcLogFormat::Legacy
    }
}

/// JDK 8 `-XX:+PrintGCDetails` markers.
fn looks_legacy(line: &str) -> bool {
    line.contains("[GC (")
        || line.contains("[Full GC (")
        || line.contains("[GC pause (")
        || line.contains("[GC remark")
        || line.contains("[GC cleanup")
        || line.contains("[GC concurrent-")
        || line.contains("[CMS-concurrent-")
        || line.contains("[PSYoungGen: ")
        || line.contains("[ParNew: ")
        || line.contains("[DefNew: ")
        || (line.starts_with("CommandLine flags:") && line.contains("-XX:+PrintGC"))
}

/// Parse one GC log file in whichever dialect it was written.
pub fn parse_log(text: &str) -> ParsedGcLog {
    match detect_format(text) {
        GcLogFormat::Unified => unified::parse(text),
        GcLogFormat::Legacy => legacy::parse(text),
        format => ParsedGcLog {
            format,
            collector: Collector::Unknown,
            jvm: JvmInfo::default(),
            events: Vec::new(),
            line_count: text.lines().count(),
            unparsed_count: 0,
            unparsed_samples: Vec::new(),
            pid: None,
            missing_timestamps: true,
            first_line_uptime_ms: None,
            last_line_uptime_ms: None,
            first_line_wall_ms: None,
            last_line_wall_ms: None,
            rotation_created: None,
            rotation_saved_as: None,
        },
    }
}
