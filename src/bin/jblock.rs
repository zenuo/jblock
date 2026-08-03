//! `jblock` CLI — analyze Java thread dumps from file, pipe, or clipboard.
//!
//! ```text
//! jstack $PID | jblock
//! jblock dump.txt
//! jblock -c
//! jblock t1.txt t2.txt --section findings
//! jblock dump.txt -j > report.json
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use jblock::cli::{
    run, CliOptions, ColorMode, OutputFormat, Section, SeverityFilter,
};

#[derive(Debug, Clone, ValueEnum)]
enum OutputArg {
    Text,
    Json,
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
    about = "Analyze Java thread dumps (file, stdin pipe, or clipboard)",
    after_help = "EXIT CODES:\n  0  clean or info-only\n  1  warning/critical findings or deadlocks\n  2  usage / I/O error\n  3  input is not a recognizable dump"
)]
struct Args {
    /// Thread dump file(s). Use `-` for stdin. Multiple files = series analysis.
    files: Vec<PathBuf>,

    /// Read dump text from the system clipboard.
    #[arg(short = 'c', long = "clipboard")]
    clipboard: bool,

    /// Output format.
    #[arg(short = 'o', long = "output", value_enum, default_value_t = OutputArg::Text)]
    output: OutputArg,

    /// Shortcut for `--output json`.
    #[arg(short = 'j', long = "json")]
    json: bool,

    /// Sections to show (repeatable): findings, summary, contention, deadlocks, threads, all.
    #[arg(short = 's', long = "section", value_name = "SECTION")]
    sections: Vec<String>,

    /// Filter thread table by state (e.g. BLOCKED).
    #[arg(long = "state", value_name = "STATE")]
    state: Option<String>,

    /// Minimum finding severity: critical | warning | info.
    #[arg(long = "severity", value_name = "LEVEL")]
    severity: Option<String>,

    /// Max rows in the thread table.
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
                    "invalid --section {s:?} (use findings|summary|contention|deadlocks|threads|all)"
                ));
            }
        }
    }
    Ok(out)
}
