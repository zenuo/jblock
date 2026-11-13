//! Host-only CLI shell around [`crate::parser`] (feat-056).
//!
//! Supports reading dumps from files, stdin pipes, or the system clipboard,
//! then rendering Findings-first text or JSON.

mod findings;
mod gc;
mod input;
mod render;

pub use findings::{aggregate_contention, build_cli_findings, CliFinding, ContentionGroup};
pub use input::{decode_text, read_clipboard, read_inputs, InputError, InputSource};
pub use gc::{exit_for_gc, render_gc_text, rule_title};
pub use render::{render_multi_text, render_text, ColorMode, OutputFormat, Section, SeverityFilter};
use render::Style;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::gc::report::build_report;
use crate::gc::stitch::{stitch, GcInput, GroupingMode};
use crate::input_kind::detect_input_kinds;
use crate::parser::{analyze, analyze_series, Analysis, MultiDumpAnalysis};

/// Which analyzer(s) the inputs go to (feat-080).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputKindArg {
    /// Detect per input; a stdout log can be both.
    #[default]
    Auto,
    Thread,
    Gc,
}

/// Parsed CLI options (clap lives in the bin; this stays library-testable).
#[derive(Debug, Clone)]
pub struct CliOptions {
    pub files: Vec<PathBuf>,
    pub clipboard: bool,
    pub output: OutputFormat,
    pub sections: Vec<Section>,
    pub state: Option<String>,
    pub severity: Option<SeverityFilter>,
    pub limit: Option<usize>,
    pub verbose: bool,
    pub hide_jvm: bool,
    pub quiet: bool,
    pub color: ColorMode,
    pub kind: InputKindArg,
    /// GC multi-file grouping.
    pub grouping: GroupingMode,
    /// Include every GC event in JSON output.
    pub events: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            clipboard: false,
            output: OutputFormat::Text,
            sections: Section::defaults(),
            state: None,
            severity: None,
            limit: None,
            verbose: false,
            hide_jvm: false,
            quiet: false,
            color: ColorMode::Auto,
            kind: InputKindArg::Auto,
            grouping: GroupingMode::Auto,
            events: false,
        }
    }
}

/// Exit status for the CLI process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliExit {
    Ok = 0,
    Problems = 1,
    Usage = 2,
    BadDump = 3,
}

impl From<CliExit> for ExitCode {
    fn from(value: CliExit) -> Self {
        ExitCode::from(value as u8)
    }
}

/// Run analysis for the given options; write to `out` / `err`.
pub fn run_with_writers<W: Write, E: Write>(
    opts: &CliOptions,
    out: &mut W,
    err: &mut E,
) -> CliExit {
    if opts.clipboard && !opts.files.is_empty() {
        let _ = writeln!(
            err,
            "error: --clipboard cannot be combined with file arguments"
        );
        return CliExit::Usage;
    }

    let sources = match read_inputs(opts) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return match e {
                InputError::Usage(_) => CliExit::Usage,
                _ => CliExit::Usage,
            };
        }
    };

    if sources.is_empty() {
        let _ = writeln!(err, "error: no dump input");
        return CliExit::Usage;
    }

    let labels: Vec<String> = sources.iter().map(|s| s.label.clone()).collect();
    let texts: Vec<&str> = sources.iter().map(|s| s.text.as_str()).collect();

    // Route inputs: GC logs to the GC analyzer, thread dumps to the thread analyzer.
    // Without any GC log, everything goes to the thread analyzer as before (feat-080).
    let kinds: Vec<_> = texts.iter().map(|t| detect_input_kinds(t)).collect();
    let (gc_idx, thread_idx): (Vec<usize>, Vec<usize>) = match opts.kind {
        InputKindArg::Thread => (Vec::new(), (0..texts.len()).collect()),
        InputKindArg::Gc => ((0..texts.len()).collect(), Vec::new()),
        InputKindArg::Auto => {
            let gc: Vec<usize> = (0..texts.len()).filter(|&i| kinds[i].gc_log).collect();
            let thread: Vec<usize> = if gc.is_empty() {
                (0..texts.len()).collect()
            } else {
                (0..texts.len()).filter(|&i| kinds[i].thread_dump).collect()
            };
            (gc, thread)
        }
    };
    if !gc_idx.is_empty() {
        for (i, k) in kinds.iter().enumerate() {
            if opts.kind == InputKindArg::Auto && k.is_unknown() {
                let _ = writeln!(err, "warning: skipping {}: not a thread dump or GC log", labels[i]);
            }
        }
        return run_mixed(opts, &texts, &labels, &thread_idx, &gc_idx, out, err);
    }

    if texts.len() == 1 {
        let analysis = analyze(texts[0]);
        finish_single(opts, &analysis, &labels[0], out, err)
    } else {
        let multi = analyze_series(&texts);
        finish_multi(opts, &multi, &labels, out, err)
    }
}

fn finish_single<W: Write, E: Write>(
    opts: &CliOptions,
    analysis: &Analysis,
    source: &str,
    out: &mut W,
    err: &mut E,
) -> CliExit {
    if analysis.format == crate::parser::DumpFormat::Unknown && analysis.total_threads == 0 {
        let _ = writeln!(
            err,
            "error: input does not look like a Java thread dump (0 threads, format=unknown)"
        );
        return CliExit::BadDump;
    }

    if !opts.quiet {
        match opts.output {
            OutputFormat::Json => {
                if let Err(e) = serde_json::to_writer_pretty(&mut *out, analysis) {
                    let _ = writeln!(err, "error: failed to write JSON: {e}");
                    return CliExit::Usage;
                }
                let _ = writeln!(out);
            }
            OutputFormat::Text => {
                let text = render_text(analysis, source, opts);
                if let Err(e) = write!(out, "{text}") {
                    let _ = writeln!(err, "error: write failed: {e}");
                    return CliExit::Usage;
                }
            }
        }
    }

    exit_for_analysis(analysis)
}

fn finish_multi<W: Write, E: Write>(
    opts: &CliOptions,
    multi: &MultiDumpAnalysis,
    labels: &[String],
    out: &mut W,
    err: &mut E,
) -> CliExit {
    for (i, dump) in multi.dumps.iter().enumerate() {
        if dump.format == crate::parser::DumpFormat::Unknown && dump.total_threads == 0 {
            let label = labels.get(i).map(String::as_str).unwrap_or("?");
            let _ = writeln!(
                err,
                "error: dump {label} does not look like a Java thread dump"
            );
            return CliExit::BadDump;
        }
    }

    if !opts.quiet {
        match opts.output {
            OutputFormat::Json => {
                if let Err(e) = serde_json::to_writer_pretty(&mut *out, multi) {
                    let _ = writeln!(err, "error: failed to write JSON: {e}");
                    return CliExit::Usage;
                }
                let _ = writeln!(out);
            }
            OutputFormat::Text => {
                let text = render_multi_text(multi, labels, opts);
                if let Err(e) = write!(out, "{text}") {
                    let _ = writeln!(err, "error: write failed: {e}");
                    return CliExit::Usage;
                }
            }
        }
    }

    let mut worst = CliExit::Ok;
    for dump in &multi.dumps {
        let code = exit_for_analysis(dump);
        if code as u8 > worst as u8 {
            worst = code;
        }
    }
    if !multi.cross_patterns.is_empty() {
        let has_warn = multi.cross_patterns.iter().any(|p| {
            p.severity == "critical" || p.severity == "warning"
        });
        if has_warn && (worst as u8) < (CliExit::Problems as u8) {
            worst = CliExit::Problems;
        }
    }
    worst
}

/// GC logs (and any thread dumps next to them): GC report after the thread report.
fn run_mixed<W: Write, E: Write>(
    opts: &CliOptions,
    texts: &[&str],
    labels: &[String],
    thread_idx: &[usize],
    gc_idx: &[usize],
    out: &mut W,
    err: &mut E,
) -> CliExit {
    let gc_labels: Vec<String> = gc_idx.iter().map(|&i| labels[i].clone()).collect();
    let inputs: Vec<GcInput> = gc_idx.iter().map(|&i| GcInput { name: &labels[i], text: texts[i] }).collect();
    let stitched = stitch(&inputs, opts.grouping);
    if stitched.groups.is_empty() {
        let _ = writeln!(err, "error: input does not look like a GC log");
        return CliExit::BadDump;
    }
    let report = build_report(&stitched);
    let thread = match thread_idx.len() {
        0 => None,
        1 => Some(analyze_series(&[texts[thread_idx[0]]])),
        _ => Some(analyze_series(&thread_idx.iter().map(|&i| texts[i]).collect::<Vec<_>>())),
    };
    let thread_labels: Vec<String> = thread_idx.iter().map(|&i| labels[i].clone()).collect();

    if !opts.quiet {
        let written = match opts.output {
            OutputFormat::Json => {
                let events: Option<Vec<&Vec<crate::gc::GcEvent>>> =
                    opts.events.then(|| stitched.groups.iter().map(|g| &g.log.events).collect());
                let value = serde_json::json!({
                    "gc": report,
                    "gc_events": events,
                    "thread": thread,
                });
                // A GC-only run without events prints the bare GcReport.
                let value = if thread.is_none() && events.is_none() { serde_json::to_value(&report).unwrap_or(value) } else { value };
                serde_json::to_writer_pretty(&mut *out, &value).map_err(|e| e.to_string()).and_then(|_| writeln!(out).map_err(|e| e.to_string()))
            }
            OutputFormat::Text => {
                let mut text = String::new();
                if let Some(t) = &thread {
                    text.push_str(&if t.dumps.len() == 1 {
                        render_text(&t.dumps[0], &thread_labels[0], opts)
                    } else {
                        render_multi_text(t, &thread_labels, opts)
                    });
                    text.push('\n');
                }
                text.push_str(&render_gc_text(&report, &stitched, &gc_labels, opts));
                write!(out, "{text}").map_err(|e| e.to_string())
            }
        };
        if let Err(e) = written {
            let _ = writeln!(err, "error: write failed: {e}");
            return CliExit::Usage;
        }
    }

    let mut worst = exit_for_gc(&report);
    if let Some(t) = &thread {
        for d in &t.dumps {
            if exit_for_analysis(d) == CliExit::Problems {
                worst = CliExit::Problems;
            }
        }
    }
    worst
}

/// `0` if clean / info-only; `1` if warning/critical findings or deadlocks.
pub fn exit_for_analysis(analysis: &Analysis) -> CliExit {
    if !analysis.deadlocks.is_empty() {
        return CliExit::Problems;
    }
    for p in &analysis.patterns {
        if p.severity == "critical" || p.severity == "warning" {
            return CliExit::Problems;
        }
    }
    let findings = build_cli_findings(analysis);
    if findings
        .iter()
        .any(|f| f.severity == "critical" || f.severity == "warning")
    {
        return CliExit::Problems;
    }
    CliExit::Ok
}

/// Convenience for the binary: write to stdout/stderr.
pub fn run(opts: &CliOptions) -> CliExit {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    run_with_writers(opts, &mut out, &mut err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const DEADLOCK: &str = include_str!("../../tests/fixtures/deadlock_real_jstack.txt");

    #[test]
    fn text_output_includes_findings_and_summary() {
        let opts = CliOptions {
            sections: Section::defaults(),
            ..CliOptions::default()
        };
        let analysis = analyze(DEADLOCK);
        let text = render_text(&analysis, "fixture", &opts);
        assert!(text.contains("FINDINGS"), "{text}");
        assert!(text.contains("DEADLOCK") || text.contains("deadlock"), "{text}");
        assert!(text.contains("SUMMARY"), "{text}");
        assert!(text.contains("jstack") || text.contains("threads"), "{text}");
    }

    #[test]
    fn json_roundtrip_via_run() {
        // Simulate file input by writing tempfile-like path through analyze path.
        let opts = CliOptions {
            output: OutputFormat::Json,
            quiet: false,
            ..CliOptions::default()
        };
        let analysis = analyze(DEADLOCK);
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let code = finish_single(&opts, &analysis, "stdin", &mut out, &mut err);
        assert_eq!(code, CliExit::Problems);
        let s = String::from_utf8(out.into_inner()).unwrap();
        assert!(s.contains("\"total_threads\""), "{s}");
        assert!(s.contains("\"deadlocks\""), "{s}");
    }

    fn fixture(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(rel)
    }

    fn run_files(files: &[&str], opts: CliOptions) -> (CliExit, String, String) {
        let opts = CliOptions { files: files.iter().map(|f| fixture(f)).collect(), color: ColorMode::Never, ..opts };
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let code = run_with_writers(&opts, &mut out, &mut err);
        (code, String::from_utf8(out.into_inner()).unwrap(), String::from_utf8(err.into_inner()).unwrap())
    }

    #[test]
    fn cli_gc_text_report_and_exit_codes() {
        let (code, out, _) = run_files(&["gc/g1-stress-jdk21.log"], CliOptions::default());
        assert_eq!(code, CliExit::Problems);
        assert!(out.contains("GC log: ") && out.contains(" · G1 · JDK 21"), "{out}");
        assert!(out.contains("GC04 critical Evacuation failure"), "{out}");
        assert!(out.contains("Summary") && out.contains("heap after GC  "), "{out}");
        assert!(out.contains("Causes") && out.contains("Longest pauses (top 10)"), "{out}");
        // A calm log is info-only → exit 0.
        let (code, out, _) = run_files(&["gc/g1-default-jdk21.log"], CliOptions::default());
        assert_eq!(code, CliExit::Ok, "{out}");
        // JDK 8 Parallel and CMS.
        let (code, out, _) = run_files(&["gc/jdk8-cms.log"], CliOptions::default());
        assert_eq!(code, CliExit::Problems);
        assert!(out.contains("GC05 critical"), "{out}");
    }

    #[test]
    fn cli_gc_rotated_segments_and_grouping() {
        let files = ["gc/g1-rotated-jdk21.log", "gc/g1-rotated-jdk21.log.2", "gc/g1-rotated-jdk21.log.0", "gc/g1-rotated-jdk21.log.1"];
        let (_, out, _) = run_files(&files, CliOptions::default());
        assert_eq!(out.matches("GC log: ").count(), 1, "{out}");
        assert!(out.contains("g1-rotated-jdk21.log.0 (+3 segments)"), "{out}");
        let (_, out, _) = run_files(&files, CliOptions { grouping: GroupingMode::Split, ..CliOptions::default() });
        assert_eq!(out.matches("GC log: ").count(), 4);
    }

    #[test]
    fn cli_gc_json_shapes() {
        let json = CliOptions { output: OutputFormat::Json, ..CliOptions::default() };
        let (_, out, _) = run_files(&["gc/parallel-jdk21.log"], json.clone());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["groups"][0]["collector"], "parallel");
        assert!(v.get("gc_events").is_none(), "bare GcReport by default");
        let (_, out, _) = run_files(&["gc/parallel-jdk21.log"], CliOptions { events: true, ..json.clone() });
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["gc_events"][0].as_array().unwrap().len(), 26);
        // Mixed thread dump + GC log: both analyses.
        let (code, out, _) = run_files(&["deadlock_real_jstack.txt", "gc/serial-jdk21.log"], json);
        assert_eq!(code, CliExit::Problems, "deadlock → 1");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["thread"]["dumps"][0]["total_threads"].as_u64().unwrap() > 0);
        assert_eq!(v["gc"]["groups"][0]["collector"], "serial");
    }

    #[test]
    fn cli_gc_mixed_text_kind_override_and_unknown_inputs() {
        let (_, out, _) = run_files(&["deadlock_real_jstack.txt", "gc/serial-jdk21.log"], CliOptions::default());
        let thread_at = out.find("deadlock").unwrap_or(usize::MAX);
        let gc_at = out.find("GC log: ").unwrap();
        assert!(thread_at < gc_at, "thread report first: {out}");
        // Forcing the thread analyzer on a GC log: not a dump.
        let (code, _, err) = run_files(&["gc/serial-jdk21.log"], CliOptions { kind: InputKindArg::Thread, ..CliOptions::default() });
        assert_eq!(code, CliExit::BadDump, "{err}");
        // Unknown text next to a GC log is skipped with a warning.
        let (code, _, err) = run_files(&["java-versions/java21-version.txt", "gc/serial-jdk21.log"], CliOptions::default());
        assert!(err.contains("warning: skipping") && err.contains("java21-version.txt"), "{err}");
        assert_eq!(code, CliExit::Ok);
        // Events section honors --limit.
        let (_, out, _) = run_files(&["gc/serial-jdk21.log"], CliOptions { sections: vec![Section::Events], limit: Some(3), ..CliOptions::default() });
        assert!(out.contains("Events (16)"), "{out}");
        assert_eq!(out.lines().filter(|l| l.contains("Pause Young")).count(), 3);
    }

    #[test]
    fn cli_gzip_inputs() {
        // One rotated segment gzipped: decompressed transparently and still stitched into one JVM.
        let files = ["gc/g1-rotated-jdk21.log.0", "gc/g1-rotated-jdk21.log.1.gz", "gc/g1-rotated-jdk21.log.2", "gc/g1-rotated-jdk21.log"];
        let (_, out, err) = run_files(&files, CliOptions::default());
        assert!(err.is_empty(), "{err}");
        assert_eq!(out.matches("GC log: ").count(), 1, "{out}");
        assert!(out.contains("(+3 segments)"), "{out}");
        assert!(matches!(decode_text(vec![0x1f, 0x8b, 0x08, 0, 1, 2], "x.gz"), Err(InputError::Io(m)) if m.contains("gunzip")));
        assert!(matches!(decode_text(vec![0xff, 0xfe, 0x00], "bin"), Err(InputError::Io(m)) if m.contains("not UTF-8")));
        assert_eq!(decode_text(b"plain".to_vec(), "t").unwrap(), "plain");
    }

    #[test]
    fn bad_dump_exit_code() {
        let opts = CliOptions::default();
        let analysis = analyze("not a dump at all");
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let code = finish_single(&opts, &analysis, "stdin", &mut out, &mut err);
        assert_eq!(code, CliExit::BadDump);
    }

    #[test]
    fn section_threads_lists_names() {
        let opts = CliOptions {
            sections: vec![Section::Threads],
            limit: Some(5),
            hide_jvm: true,
            ..CliOptions::default()
        };
        let analysis = analyze(DEADLOCK);
        let text = render_text(&analysis, "fixture", &opts);
        assert!(text.contains("THREADS"), "{text}");
        // Deadlock participants are app threads, not JVM noise.
        assert!(
            text.contains("deadlock-0")
                || text.contains("deadlock-1")
                || text.contains("deadlock-2"),
            "{text}"
        );
    }

    #[test]
    fn clipboard_with_files_is_usage_error() {
        let opts = CliOptions {
            clipboard: true,
            files: vec![PathBuf::from("a.txt")],
            ..CliOptions::default()
        };
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let code = run_with_writers(&opts, &mut out, &mut err);
        assert_eq!(code, CliExit::Usage);
    }
}
