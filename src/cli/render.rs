//! Text / JSON rendering for the CLI (feat-056).

use crate::parser::{Analysis, DumpFormat, MultiDumpAnalysis, ThreadInfo};

use super::findings::{aggregate_contention, build_cli_findings, is_jvm_noise, CliFinding};
use super::CliOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityFilter {
    Critical,
    Warning,
    Info,
}

impl SeverityFilter {
    pub fn allows(self, severity: &str) -> bool {
        match self {
            SeverityFilter::Info => true,
            SeverityFilter::Warning => severity == "warning" || severity == "critical",
            SeverityFilter::Critical => severity == "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "critical" => Some(Self::Critical),
            "warning" => Some(Self::Warning),
            "info" => Some(Self::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Findings,
    Summary,
    Contention,
    Deadlocks,
    Threads,
}

impl Section {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::Findings,
            Self::Summary,
            Self::Contention,
            Self::Deadlocks,
        ]
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "findings" => Some(Self::Findings),
            "summary" => Some(Self::Summary),
            "contention" => Some(Self::Contention),
            "deadlocks" => Some(Self::Deadlocks),
            "threads" => Some(Self::Threads),
            "all" => None, // handled by caller as expand-all
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Findings,
            Self::Summary,
            Self::Contention,
            Self::Deadlocks,
            Self::Threads,
        ]
    }
}

struct Style {
    color: bool,
}

impl Style {
    fn from_opts(opts: &CliOptions) -> Self {
        let color = match opts.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => std::io::IsTerminal::is_terminal(&std::io::stdout()),
        };
        Self { color }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    fn sev_mark(&self, severity: &str) -> String {
        match severity {
            "critical" => self.paint("31", "x"),
            "warning" => self.paint("33", "!"),
            _ => self.paint("36", "i"),
        }
    }
}

/// Render a single-dump text report.
pub fn render_text(analysis: &Analysis, source: &str, opts: &CliOptions) -> String {
    let style = Style::from_opts(opts);
    let mut out = String::new();
    out.push_str(&header_line(analysis, source, &style));
    out.push('\n');

    let sections = if opts.sections.is_empty() {
        Section::defaults()
    } else {
        opts.sections.clone()
    };

    for section in &sections {
        match section {
            Section::Findings => {
                out.push_str(&render_findings(analysis, opts, &style));
            }
            Section::Summary => {
                out.push_str(&render_summary(analysis, &style));
            }
            Section::Contention => {
                out.push_str(&render_contention(analysis, &style));
            }
            Section::Deadlocks => {
                out.push_str(&render_deadlocks(analysis, &style));
            }
            Section::Threads => {
                out.push_str(&render_threads(analysis, opts, &style));
            }
        }
    }
    out
}

/// Render multi-dump series (per-dump blocks + cross patterns).
pub fn render_multi_text(
    multi: &MultiDumpAnalysis,
    labels: &[String],
    opts: &CliOptions,
) -> String {
    let style = Style::from_opts(opts);
    let mut out = String::new();
    out.push_str(&style.bold(&format!(
        "jblock · {} dumps (series)",
        multi.dumps.len()
    )));
    out.push('\n');

    if !multi.cross_patterns.is_empty() {
        out.push('\n');
        out.push_str(&style.bold("CROSS-DUMP PATTERNS"));
        out.push('\n');
        for p in &multi.cross_patterns {
            if let Some(min) = opts.severity {
                if !min.allows(&p.severity) {
                    continue;
                }
            }
            let mark = style.sev_mark(&p.severity);
            out.push_str(&format!(
                "  {mark} {} ({})\n      {}\n",
                serde_kind(&p.kind),
                p.severity,
                p.detail
            ));
        }
    }

    for (i, dump) in multi.dumps.iter().enumerate() {
        let label = labels.get(i).map(String::as_str).unwrap_or("?");
        out.push('\n');
        out.push_str(&style.bold(&format!("── dump {} · {label} ──", i + 1)));
        out.push('\n');
        // Avoid duplicating the outer header color decision; nest with source label.
        let nested = render_text(dump, label, opts);
        out.push_str(&nested);
    }
    out
}

fn header_line(analysis: &Analysis, source: &str, style: &Style) -> String {
    let fmt = format_label(analysis.format);
    let mut parts = vec![
        "jblock".to_string(),
        fmt.to_string(),
        format!("{} threads", analysis.total_threads),
    ];
    if let Some(ref v) = analysis.java_version {
        parts.push(format!("Java {v}"));
    }
    let head = style.bold(&parts.join(" · "));
    format!("{head}\nsource: {source}\n")
}

fn format_label(f: DumpFormat) -> &'static str {
    match f {
        DumpFormat::Jstack => "jstack",
        DumpFormat::ThreadMxBean => "thread-mx-bean",
        DumpFormat::ThreadDumpJson => "thread-dump-json",
        DumpFormat::Unknown => "unknown",
    }
}

fn render_findings(analysis: &Analysis, opts: &CliOptions, style: &Style) -> String {
    let mut findings = build_cli_findings(analysis);
    if let Some(min) = opts.severity {
        findings.retain(|f| min.allows(&f.severity));
    }

    let mut out = String::new();
    out.push('\n');
    out.push_str(&style.bold("FINDINGS"));
    out.push('\n');
    if findings.is_empty() {
        let ok = match &analysis.java_version {
            Some(v) => format!("  OK  no problem findings (Java {v})"),
            None => "  OK  no problem findings".to_string(),
        };
        out.push_str(&style.paint("32", &ok));
        out.push('\n');
        return out;
    }
    for f in &findings {
        out.push_str(&format_finding(f, style));
    }
    out
}

fn format_finding(f: &CliFinding, style: &Style) -> String {
    let mark = style.sev_mark(&f.severity);
    let title = style.bold(&format!("{} ({})", f.kind.to_uppercase(), f.severity));
    format!("  {mark} {title}\n      {}\n      {}\n", f.title, f.detail)
}

fn render_summary(analysis: &Analysis, style: &Style) -> String {
    let mut out = String::new();
    out.push('\n');
    out.push_str(&style.bold("SUMMARY"));
    out.push('\n');
    if analysis.state_counts.is_empty() {
        out.push_str("  (no state counts)\n");
        return out;
    }
    let line = analysis
        .state_counts
        .iter()
        .map(|s| format!("{} {}", s.state, s.count))
        .collect::<Vec<_>>()
        .join("  ");
    out.push_str(&format!("  {line}\n"));
    out
}

fn render_contention(analysis: &Analysis, style: &Style) -> String {
    let groups = aggregate_contention(&analysis.blocked_edges);
    let mut out = String::new();
    out.push('\n');
    out.push_str(&style.bold("CONTENTION"));
    out.push('\n');
    if groups.is_empty() {
        out.push_str("  (none)\n");
        return out;
    }
    for g in groups.iter().take(10) {
        out.push_str(&format!(
            "  lock {}  owner={}  waiters={}\n",
            g.lock,
            g.owner_thread.as_deref().unwrap_or("unknown"),
            g.waiters.len()
        ));
        for w in g.waiters.iter().take(8) {
            out.push_str(&format!("    <- {w}\n"));
        }
        if g.waiters.len() > 8 {
            out.push_str(&format!("    … {} more\n", g.waiters.len() - 8));
        }
    }
    out
}

fn render_deadlocks(analysis: &Analysis, style: &Style) -> String {
    let mut out = String::new();
    out.push('\n');
    out.push_str(&style.bold("DEADLOCKS"));
    out.push('\n');
    if analysis.deadlocks.is_empty() {
        out.push_str("  (none)\n");
        return out;
    }
    for (i, d) in analysis.deadlocks.iter().enumerate() {
        let cycle = if d.threads.is_empty() {
            String::new()
        } else {
            format!("{} → {}", d.threads.join(" → "), d.threads[0])
        };
        out.push_str(&format!("  cycle #{}: {cycle}\n", i + 1));
        for e in &d.edges {
            out.push_str(&format!(
                "    {} waits on {} held by {}\n",
                e.blocked_thread,
                e.lock,
                e.owner_thread.as_deref().unwrap_or("unknown")
            ));
        }
    }
    out
}

fn render_threads(analysis: &Analysis, opts: &CliOptions, style: &Style) -> String {
    let mut threads: Vec<&ThreadInfo> = analysis.threads.iter().collect();
    if opts.hide_jvm {
        threads.retain(|t| !is_jvm_noise(&t.name));
    }
    if let Some(ref state) = opts.state {
        let want = state.to_ascii_uppercase();
        threads.retain(|t| t.state.eq_ignore_ascii_case(&want));
    }

    let mut out = String::new();
    out.push('\n');
    out.push_str(&style.bold("THREADS"));
    out.push('\n');
    out.push_str(&format!(
        "  {:<16} {:<10} {:>5}  {}\n",
        "STATE", "KIND", "DEPTH", "NAME"
    ));

    let limit = opts.limit.unwrap_or(usize::MAX);
    let shown = threads.len().min(limit);
    for t in threads.iter().take(shown) {
        let kind = match t.kind {
            crate::parser::ThreadKind::Platform => "platform",
            crate::parser::ThreadKind::Virtual => "virtual",
            crate::parser::ThreadKind::Carrier => "carrier",
        };
        out.push_str(&format!(
            "  {:<16} {:<10} {:>5}  {}\n",
            t.state, kind, t.stack_depth, t.name
        ));
        if opts.verbose {
            for frame in &t.stack {
                out.push_str(&format!("      at {frame}\n"));
            }
        }
    }
    if threads.len() > shown {
        out.push_str(&format!(
            "  … {} more (use --limit or omit --limit)\n",
            threads.len() - shown
        ));
    }
    out
}

fn serde_kind(kind: &crate::parser::PatternKind) -> String {
    // Keep display aligned with serde kebab-case.
    use crate::parser::PatternKind::*;
    match kind {
        ThreadPoolExhaustion => "thread-pool-exhaustion",
        SyncIoHotspot => "sync-io-hotspot",
        DangerousHotLockOwner => "dangerous-hot-lock-owner",
        ConnectionPoolBorrow => "connection-pool-borrow",
        FutureLatchWaitTree => "future-latch-wait-tree",
        LoggingAppenderContention => "logging-appender-contention",
        BusyWaitSpinHotspot => "busy-wait-spin-hotspot",
        ConditionParkStarvation => "condition-park-starvation",
        LockOrderInconsistency => "lock-order-inconsistency",
        FinalizerPressure => "finalizer-pressure",
        SleepAsScheduler => "sleep-as-scheduler",
        FrameworkPoolSaturation => "framework-pool-saturation",
        DnsResolutionStall => "dns-resolution-stall",
        ThreadLeak => "thread-leak",
        Livelock => "livelock",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::analyze;

    #[test]
    fn parse_section_and_severity() {
        assert_eq!(Section::parse("findings"), Some(Section::Findings));
        assert_eq!(Section::parse("ALL"), None);
        assert_eq!(SeverityFilter::parse("WARNING"), Some(SeverityFilter::Warning));
        assert!(SeverityFilter::Warning.allows("critical"));
        assert!(!SeverityFilter::Critical.allows("warning"));
    }

    #[test]
    fn ok_empty_findings_when_clean() {
        // Minimal non-empty jstack-like content with one RUNNABLE thread, no contention.
        let dump = r#"
"main" #1 prio=5
   java.lang.Thread.State: RUNNABLE
	at demo.Main.main(Main.java:1)
"#;
        let a = analyze(dump);
        let opts = CliOptions {
            sections: vec![Section::Findings],
            color: ColorMode::Never,
            ..CliOptions::default()
        };
        let text = render_text(&a, "t", &opts);
        assert!(text.contains("OK"), "{text}");
        assert!(text.contains("FINDINGS"), "{text}");
    }
}
