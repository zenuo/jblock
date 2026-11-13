//! `jblock` CLI — analyze Java thread dumps and GC logs from file, pipe, or clipboard.
//!
//! ```text
//! jstack $PID | jblock
//! jblock dump.txt
//! jblock -c
//! jblock t1.txt t2.txt --section findings
//! jblock dump.txt -j > report.json
//! jblock gc.log gc.log.0 gc.log.1          # rotated GC log segments, stitched
//! jblock gc.log -s pauses -n 20
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use jblock::cli::{
    run, CliOptions, ColorMode, InputKindArg, OutputFormat, Section, SeverityFilter,
};
use jblock::gc::stitch::GroupingMode;

#[derive(Debug, Clone, ValueEnum)]
enum OutputArg {
    Text,
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
enum KindArg {
    Auto,
    Thread,
    Gc,
}

#[derive(Debug, Clone, ValueEnum)]
enum GroupingArg {
    Auto,
    Merge,
    Split,
}

#[derive(Debug, Clone, ValueEnum)]
enum ColorArg {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Parser)]
#[command(
    name = "jblock",
    version,
    about = "Analyze Java thread dumps and GC logs (file, stdin pipe, or clipboard)",
    after_help = "INPUT:\n  FILE(s)              thread dumps (multiple = series analysis) and/or GC logs\n                       (JDK 8 PrintGCDetails or JDK 9+ -Xlog:gc*; rotated segments are stitched)\n  stdin pipe           jstack $PID | jblock\n  -c / --clipboard     paste text from the OS clipboard\n                       (pbpaste, wl-paste, xclip/xsel, or PowerShell Get-Clipboard)\n\nGC SECTIONS:\n  findings, summary, causes, pauses (longest), events (all; -n limits)\n\nEXIT CODES:\n  0  clean or info-only\n  1  warning/critical findings or deadlocks\n  2  usage / I/O error\n  3  input is not a recognizable dump or GC log"
)]
struct Args {
    /// Thread dump / GC log file(s). Use `-` for stdin. Multiple dumps = series analysis.
    files: Vec<PathBuf>,

    /// Input type: auto-detect per input, or force thread dumps / GC logs.
    #[arg(long = "kind", value_enum, default_value_t = KindArg::Auto)]
    kind: KindArg,

    /// GC log files: auto (stitch rotated segments of one JVM), merge (one JVM), split (one per file).
    #[arg(long = "grouping", value_enum, default_value_t = GroupingArg::Auto)]
    grouping: GroupingArg,

    /// JSON output: include every GC event.
    #[arg(long = "events")]
    events: bool,

    /// Read dump text from the system clipboard.
    #[arg(short = 'c', long = "clipboard")]
    clipboard: bool,

    /// Output format.
    #[arg(short = 'o', long = "output", value_enum, default_value_t = OutputArg::Text)]
    output: OutputArg,

    /// Shortcut for `--output json`.
    #[arg(short = 'j', long = "json")]
    json: bool,

    /// Sections to show (repeatable): findings, summary, contention, deadlocks, threads,
    /// causes, pauses, events, all.
    #[arg(short = 's', long = "section", value_name = "SECTION")]
    sections: Vec<String>,

    /// Filter thread table by state (e.g. BLOCKED).
    #[arg(long = "state", value_name = "STATE")]
    state: Option<String>,

    /// Minimum finding severity: critical | warning | info.
    #[arg(long = "severity", value_name = "LEVEL")]
    severity: Option<String>,

    /// Max rows in the thread table / GC causes, pauses and events.
    #[arg(short = 'n', long = "limit", value_name = "N")]
    limit: Option<usize>,

    /// Include full stack frames in the thread table.
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Hide common JVM / HotSpot noise threads.
    #[arg(long = "hide-jvm")]
    hide_jvm: bool,

    /// Suppress report stdout (exit code only).
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Colorize text output.
    #[arg(long = "color", value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match build_options(args) {
        Ok(opts) => run(&opts).into(),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}

fn build_options(args: Args) -> Result<CliOptions, String> {
    let output = if args.json {
        OutputFormat::Json
    } else {
        match args.output {
            OutputArg::Text => OutputFormat::Text,
            OutputArg::Json => OutputFormat::Json,
        }
    };

    let sections = parse_sections(&args.sections)?;
    let severity = match args.severity {
        None => None,
        Some(ref s) => Some(
            SeverityFilter::parse(s)
                .ok_or_else(|| format!("invalid --severity {s:?} (use critical|warning|info)"))?,
        ),
    };

    let color = match args.color {
        ColorArg::Auto => ColorMode::Auto,
        ColorArg::Always => ColorMode::Always,
        ColorArg::Never => ColorMode::Never,
    };

    Ok(CliOptions {
        files: args.files,
        clipboard: args.clipboard,
        output,
        sections,
        state: args.state,
        severity,
        limit: args.limit,
        verbose: args.verbose,
        hide_jvm: args.hide_jvm,
        quiet: args.quiet,
        color,
        kind: match args.kind {
            KindArg::Auto => InputKindArg::Auto,
            KindArg::Thread => InputKindArg::Thread,
            KindArg::Gc => InputKindArg::Gc,
        },
        grouping: match args.grouping {
            GroupingArg::Auto => GroupingMode::Auto,
            GroupingArg::Merge => GroupingMode::Merge,
            GroupingArg::Split => GroupingMode::Split,
        },
        events: args.events,
    })
}

fn parse_sections(raw: &[String]) -> Result<Vec<Section>, String> {
    if raw.is_empty() {
        return Ok(Section::defaults());
    }
    let mut out = Vec::new();
    for s in raw {
        if s.eq_ignore_ascii_case("all") {
            return Ok(Section::all());
        }
        match Section::parse(s) {
            Some(sec) => {
                if !out.contains(&sec) {
                    out.push(sec);
                }
            }
            None => {
                return Err(format!(
                    "invalid --section {s:?} (use findings|summary|contention|deadlocks|threads|causes|pauses|events|all)"
                ));
            }
        }
    }
    Ok(out)
}
