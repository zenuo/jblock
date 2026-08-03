//! Host-only CLI shell around [`crate::parser`] (feat-056).
//!
//! Supports reading dumps from files, stdin pipes, or the system clipboard,
//! then rendering Findings-first text or JSON.

mod findings;
mod input;
mod render;

pub use findings::{aggregate_contention, build_cli_findings, CliFinding, ContentionGroup};
pub use input::{read_clipboard, read_inputs, InputError, InputSource};
pub use render::{render_multi_text, render_text, ColorMode, OutputFormat, Section, SeverityFilter};

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::parser::{analyze, analyze_series, Analysis, MultiDumpAnalysis};

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
