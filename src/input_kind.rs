//! Classify an input text as thread dump and/or GC log (feat-074).
//!
//! A file can be both: a `catalina.out` may hold a `kill -3` thread dump and
//! `-Xlog:gc` output written to stdout, so each kind is detected independently.

use serde::Serialize;

use crate::gc::{self, GcLogFormat};

/// Lines inspected per input; enough to get past application start-up chatter.
const SNIFF_LINES: usize = 5000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct InputKinds {
    pub thread_dump: bool,
    pub gc_log: bool,
}

impl InputKinds {
    pub fn is_unknown(self) -> bool {
        !self.thread_dump && !self.gc_log
    }
}

/// Real thread-stack headers: `"name" #12 …` (jstack) or `"name" Id=13 …` (ThreadMXBean).
fn is_thread_header(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('"') else {
        return false;
    };
    let Some(end) = rest.find('"') else {
        return false;
    };
    let after = rest[end + 1..].trim_start();
    after.starts_with('#') || after.starts_with("daemon") || after.starts_with("Id=")
}

/// Detect which analyzers `text` should be fed to.
pub fn detect_input_kinds(text: &str) -> InputKinds {
    let trimmed = text.trim_start();
    let json_dump = trimmed.starts_with('{') && trimmed.contains("\"threadDump\"");
    let mut headers = 0;
    let mut has_state_line = false;
    let mut has_full_dump_banner = false;
    for line in text.lines().take(SNIFF_LINES) {
        if is_thread_header(line) {
            headers += 1;
        } else if line.contains("java.lang.Thread.State:") {
            has_state_line = true;
        } else if line.starts_with("Full thread dump") {
            has_full_dump_banner = true;
        }
    }
    let thread_dump = json_dump || has_full_dump_banner || (headers > 0 && has_state_line) || headers >= 2;
    let sniff: String = text.lines().take(SNIFF_LINES).flat_map(|l| [l, "\n"]).collect();
    let gc_log = gc::detect_format(&sniff) != GcLogFormat::Unknown;
    InputKinds { thread_dump, gc_log }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_kinds_over_fixtures() {
        let dumps = [
            include_str!("../tests/fixtures/deadlock_real_jstack.txt"),
            include_str!("../tests/fixtures/mxbean_real_contention.txt"),
            include_str!("../tests/fixtures/java-versions/java8-jstack-contention.txt"),
            include_str!("../tests/fixtures/java-versions/java21-mxbean-contention.txt"),
            include_str!("../tests/fixtures/virtual-threads/dump_to_file.json"),
        ];
        for d in dumps {
            assert_eq!(detect_input_kinds(d), InputKinds { thread_dump: true, gc_log: false });
        }
        let gcs = [
            include_str!("../tests/fixtures/gc/g1-stress-jdk21.log"),
            include_str!("../tests/fixtures/gc/g1-gc-only-jdk21.log"),
            include_str!("../tests/fixtures/gc/parallel-jdk21.log"),
            include_str!("../tests/fixtures/gc/g1-rotated-jdk21.log.1"),
        ];
        for g in gcs {
            assert_eq!(detect_input_kinds(g), InputKinds { thread_dump: false, gc_log: true });
        }
        assert!(detect_input_kinds("hello\nworld").is_unknown());
        assert!(detect_input_kinds("").is_unknown());
    }

    #[test]
    fn input_kinds_stdout_with_both() {
        let mixed = format!(
            "app starting\n{}\n{}\n",
            include_str!("../tests/fixtures/gc/g1-default-jdk21.log"),
            include_str!("../tests/fixtures/deadlock_real_jstack.txt")
        );
        assert_eq!(detect_input_kinds(&mixed), InputKinds { thread_dump: true, gc_log: true });
    }
}
