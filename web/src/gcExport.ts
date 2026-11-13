/**
 * Standalone HTML export of a GC report (feat-079): reuses the app stylesheet
 * (`?inline`) and markup classes like the thread-dump export, with static SVG
 * charts from `gcSvg.ts`. No JavaScript in the output.
 */

import { CHROME, HEAP_COLORS, SERIES_COLORS, worstRow, type TimeAxis } from "./gcCharts";
import { findingText, type Translate } from "./gcFindings";
import { collectorLabel, formatKb, formatMs, formatPct, formatRate, formatWall } from "./gcFormat";
import { heapSvg, histogramSvg, metaspaceSvg, pauseSvg, rateSvg } from "./gcSvg";
import type { GcFinding, GcReport, GcSeries, GcStats, GroupReport } from "./gcTypes";
import { htmlLangFor, type Locale, type MessageKey, type TranslateFn } from "./i18n";
import appCss from "./index.css?inline";
import { NAV_LABEL_KEYS, gcReportSections, sectionDomId, type ReportSectionId } from "./reportNav";

export interface GcExportGroup {
  group: GroupReport;
  series: GcSeries | null;
  /** KPIs for the selected range, when one is active. */
  rangeStats: GcStats | null;
}

export interface GcExportInput {
  report: GcReport;
  names: string[];
  groups: GcExportGroup[];
  axis: TimeAxis;
  /** Selected range label (already formatted), when one is active. */
  rangeLabel: string | null;
  t: TranslateFn;
  locale: Locale;
}

function esc(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

const fullCount = (s: GcStats) => s.kinds.find((k) => k.kind === "full")?.count ?? 0;

function groupLabel(g: GroupReport, names: string[]): string {
  const first = names[g.files[0]] ?? g.name;
  return g.files.length > 1 ? `${first} (+${g.files.length - 1})` : first;
}

function kpiTiles(stats: GcStats, t: TranslateFn): string {
  const full = fullCount(stats);
  const tiles: [MessageKey, string, boolean][] = [
    ["gc.kpi.throughput", formatPct(stats.throughput_pct), false],
    ["gc.kpi.p99", formatMs(stats.pause?.p99), false],
    ["gc.kpi.max", formatMs(stats.pause?.max), false],
    ["gc.kpi.totalPause", formatMs(stats.total_pause_ms), false],
    ["gc.kpi.pauses", String(stats.pause_count), false],
    ["gc.kpi.full", String(full), full > 0],
    ["gc.kpi.span", formatMs(stats.span_ms), false],
    ["gc.kpi.alloc", formatRate(stats.allocation?.avg_mb_per_s), false],
    ["gc.kpi.heapPeak", formatPct(stats.heap_after_peak_pct), false],
  ];
  return `<section class="gc-kpis">${tiles
    .map(([k, v, danger]) => `<div class="stat${danger ? " danger" : ""}"><span class="stat-value">${esc(v)}</span><span class="stat-label">${esc(t(k))}</span></div>`)
    .join("")}</section>`;
}

const SEV_MARK: Record<string, string> = { critical: "⛔", warning: "⚠️", info: "ℹ️" };

function findingsPanel(findings: GcFinding[], eventCount: number, t: TranslateFn, id: string): string {
  const body =
    findings.length === 0
      ? `<div class="finding finding-ok"><div class="finding-row"><strong><span class="finding-ok-mark" aria-hidden="true">✅</span>${esc(t("gc.findings.okTitle"))}</strong></div><span class="mono">${esc(t("gc.findings.okDetail", { count: eventCount }))}</span></div>`
      : `<ul class="findings-list">${findings
          .map((f) => {
            const text = findingText(f, t as Translate);
            return `<li class="finding finding-${f.severity}"><div class="finding-row"><strong><span class="gc-sev" aria-hidden="true">${SEV_MARK[f.severity] ?? ""}</span><span class="gc-sev-label gc-sev-${f.severity}">${esc(t(`gc.sev.${f.severity}` as MessageKey))}</span>${esc(text.title)}</strong><span class="gc-finding-actions"><span class="mono gc-rule-id">${esc(f.rule)}</span></span></div><p class="gc-finding-body">${esc(text.body)}</p><p class="gc-finding-fix"><span class="gc-finding-fix-label">${esc(t("gc.findings.fixLabel"))}</span>${esc(text.fix)}</p></li>`;
          })
          .join("")}</ul>`;
  return `<section class="panel findings" id="${id}"><h2>${esc(t("gc.findings.title"))}</h2>${body}</section>`;
}

function legend(items: [string, string, "dot" | "tri" | "line" | "band"][]): string {
  return `<ul class="gc-legend">${items
    .map(([color, label, shape]) => `<li><span class="gc-legend-key ${shape}" style="background:${shape === "band" ? CHROME.band : color}"></span>${esc(label)}</li>`)
    .join("")}</ul>`;
}

function chartPanel(id: string, title: string, legendHtml: string, svg: string, t: TranslateFn): string {
  return `<section class="panel" id="${id}"><h2>${esc(title)}</h2>${legendHtml}${svg || `<p class="empty">${esc(t("gc.noEvents"))}</p>`}</section>`;
}

function causesHtml(stats: GcStats): string {
  const rows = stats.causes.slice(0, 10);
  const max = Math.max(...rows.map((c) => c.total_pause_ms), 1e-9);
  return `<ul class="gc-causes">${rows
    .map(
      (c) =>
        `<li class="gc-cause"><span class="gc-cause-name">${esc(c.cause)}</span><span class="gc-cause-track"><span class="gc-cause-bar" style="width:${Math.max((70 * c.total_pause_ms) / max, 0.4)}%;background:${CHROME.secondary}"></span><span class="gc-cause-value">${esc(formatMs(c.total_pause_ms))} · ${c.count}×</span></span></li>`,
    )
    .join("")}</ul>`;
}

function jvmPanel(g: GroupReport, t: TranslateFn, id: string): string {
  const fmt: Record<string, MessageKey> = { unified: "gc.format.unified", legacy: "gc.format.legacy", unknown: "gc.format.unknown" };
  const rows: [MessageKey, string | null][] = [
    ["gc.jvm.collector", collectorLabel(g.collector)],
    ["gc.jvm.format", t(fmt[g.format])],
    ["gc.jvm.version", g.jvm.version],
    ["gc.jvm.heapMax", g.jvm.heap_max_kb != null ? formatKb(g.jvm.heap_max_kb) : null],
    ["gc.jvm.heapInitial", g.jvm.heap_initial_kb != null ? formatKb(g.jvm.heap_initial_kb) : null],
    ["gc.jvm.regionSize", g.jvm.region_size_kb != null ? formatKb(g.jvm.region_size_kb) : null],
    ["gc.jvm.commandLine", g.jvm.command_line],
  ];
  return `<section class="panel gc-jvm" id="${id}"><h2>${esc(t("gc.jvmInfo"))}</h2><dl class="gc-facts">${rows
    .filter(([, v]) => v)
    .map(([k, v]) => `<div class="gc-fact"><dt>${esc(t(k))}</dt><dd${k === "gc.jvm.commandLine" ? ' class="mono"' : ""}>${esc(v ?? "")}</dd></div>`)
    .join("")}</dl></section>`;
}

function diagnosticsPanel(g: GroupReport, report: GcReport, names: string[], t: TranslateFn, id: string): string {
  const rows = g.files
    .map((i) => {
      const f = report.files[i];
      return `<tr><td class="mono">${esc(names[i] ?? f.name)}</td><td>${esc(f.format)}</td><td>${esc(collectorLabel(f.collector))}</td><td class="num">${f.event_count}</td><td class="mono">${f.first_gc_id != null ? `${f.first_gc_id}–${f.last_gc_id}` : "—"}</td><td class="num">${f.unparsed_count}</td></tr>`;
    })
    .join("");
  return `<section class="panel gc-diagnostics" id="${id}"><h2>${esc(t("gc.diagnostics"))}</h2><div class="table-scroll"><table class="gc-files"><thead><tr><th>${esc(t("gc.diag.file"))}</th><th>${esc(t("gc.diag.format"))}</th><th>${esc(t("gc.diag.collector"))}</th><th class="num">${esc(t("gc.diag.events"))}</th><th>${esc(t("gc.diag.gcIds"))}</th><th class="num">${esc(t("gc.diag.unparsed"))}</th></tr></thead><tbody>${rows}</tbody></table></div></section>`;
}

function pauseLegend(t: TranslateFn): string {
  return legend([
    [SERIES_COLORS.young, t("gc.series.young"), "dot"],
    [SERIES_COLORS.mixed, t("gc.series.mixed"), "dot"],
    [SERIES_COLORS.full, t("gc.series.full"), "tri"],
    [CHROME.muted, t("gc.series.other"), "dot"],
    [CHROME.band, t("gc.series.concurrent"), "band"],
  ]);
}

function heapLegend(t: TranslateFn): string {
  return legend([
    [HEAP_COLORS.used, t("gc.series.heapUsed"), "line"],
    [HEAP_COLORS.old, t("gc.series.oldAfter"), "line"],
    [CHROME.muted, t("gc.series.capacity"), "line"],
    [SERIES_COLORS.full, t("gc.series.full"), "line"],
  ]);
}

function navHtml(ids: ReportSectionId[], t: TranslateFn): string {
  return `<input type="checkbox" id="report-nav-collapsed" class="report-nav-state" hidden /><nav class="report-nav" aria-label="${esc(t("nav.title"))}"><p class="report-nav-heading">${esc(t("nav.title"))}</p><ul class="report-nav-list">${ids
    .map((id) => `<li><a class="report-nav-link" href="#${sectionDomId(id)}">${esc(t(NAV_LABEL_KEYS[id]))}</a></li>`)
    .join("")}</ul><label class="report-nav-toggle" for="report-nav-collapsed"><span class="nav-collapse-label">${esc(t("nav.collapse"))}</span><span class="nav-expand-label">${esc(t("nav.expand"))}</span></label></nav>`;
}

function singleGroupBody(x: GcExportGroup, input: GcExportInput): { nav: ReportSectionId[]; html: string } {
  const { t, axis, report, names } = input;
  const g = x.group;
  const s = x.series;
  const off = g.wall_offset_ms;
  const parts: string[] = [];
  parts.push(`<section id="${sectionDomId("gc-overview")}">${kpiTiles(g.stats, t)}${
    x.rangeStats && input.rangeLabel ? `<p class="gc-kpi-caption">${esc(`${t("gc.kpiLabel")} ${t("gc.kpiRange")} · ${input.rangeLabel}`)}</p>${kpiTiles(x.rangeStats, t)}` : ""
  }</section>`);
  parts.push(findingsPanel(g.findings, g.event_count, t, sectionDomId("gc-findings")));
  if (s && g.event_count > 0) {
    parts.push(chartPanel(sectionDomId("gc-pauses"), t("gc.chart.pauses"), pauseLegend(t), pauseSvg(s, axis, off, t("gc.chart.pauses")), t));
    parts.push(chartPanel(sectionDomId("gc-heap"), t("gc.chart.heap"), heapLegend(t), heapSvg(s, axis, off, t("gc.chart.heap")), t));
    parts.push(
      `<div class="gc-dist-grid" id="${sectionDomId("gc-distribution")}"><section class="panel"><h2>${esc(t("gc.chart.histogram"))}</h2>${
        g.stats.pause
          ? `<p class="gc-hist-caption">${esc(t("gc.hist.percentiles", { p50: formatMs(g.stats.pause.p50), p95: formatMs(g.stats.pause.p95), p99: formatMs(g.stats.pause.p99), max: formatMs(g.stats.pause.max) }))}</p>`
          : ""
      }${histogramSvg(g.stats.histogram, t("gc.chart.histogram"))}</section><section class="panel"><h2>${esc(t("gc.chart.causes"))}</h2>${causesHtml(g.stats)}</section></div>`,
    );
    parts.push(
      chartPanel(
        sectionDomId("gc-rates"),
        t("gc.chart.rates"),
        legend([
          [HEAP_COLORS.used, t("gc.series.allocation"), "line"],
          [HEAP_COLORS.old, t("gc.series.promotion"), "line"],
        ]),
        rateSvg(s, axis, off, t("gc.chart.rates")),
        t,
      ),
    );
    parts.push(
      chartPanel(
        sectionDomId("gc-metaspace"),
        t("gc.chart.metaspace"),
        legend([
          [HEAP_COLORS.used, t("gc.series.metaspaceUsed"), "line"],
          [CHROME.muted, t("gc.series.metaspaceCommitted"), "line"],
        ]),
        metaspaceSvg(s, axis, off, t("gc.chart.metaspace")),
        t,
      ),
    );
  }
  parts.push(jvmPanel(g, t, sectionDomId("gc-jvm")));
  parts.push(diagnosticsPanel(g, report, names, t, sectionDomId("gc-diagnostics")));
  const nav = gcReportSections({ hasEvents: g.event_count > 0 }).map((x) => x.id).filter((id) => id !== "gc-events");
  return { nav, html: parts.join("\n") };
}

function compareBody(input: GcExportInput): string {
  const { t, names, axis } = input;
  const statsOf = (x: GcExportGroup) => x.rangeStats ?? x.group.stats;
  const cols: [MessageKey, (s: GcStats) => number | null, (v: number | null) => string, "high" | "low"][] = [
    ["gc.kpi.throughput", (s) => s.throughput_pct, formatPct, "low"],
    ["gc.kpi.p99", (s) => s.pause?.p99 ?? null, formatMs, "high"],
    ["gc.kpi.max", (s) => s.pause?.max ?? null, formatMs, "high"],
    ["gc.kpi.totalPause", (s) => s.total_pause_ms, formatMs, "high"],
    ["gc.kpi.full", (s) => fullCount(s), (v) => (v == null ? "—" : String(v)), "high"],
    ["gc.kpi.alloc", (s) => s.allocation?.avg_mb_per_s ?? null, formatRate, "high"],
    ["gc.kpi.heapPeak", (s) => s.heap_after_peak_pct, formatPct, "high"],
  ];
  const rows = input.groups
    .map((x, row) => {
      const cells = cols
        .map(([, get, fmt, worse]) => {
          const values = input.groups.map((y) => get(statsOf(y)));
          const worst = worstRow(values, worse) === row;
          return `<td class="num${worst ? " gc-worst" : ""}">${esc(fmt(values[row]))}${worst ? `<span class="gc-worst-badge">${esc(t("gc.compare.worst"))}</span>` : ""}</td>`;
        })
        .join("");
      return `<tr><td><span class="mono">${esc(groupLabel(x.group, names))}</span><span class="gc-compare-collector"> · ${esc(collectorLabel(x.group.collector))}</span></td>${cells}</tr>`;
    })
    .join("");
  const table = `<section class="panel" id="${sectionDomId("gc-overview")}"><h2>${esc(t("gc.compare.kpis"))}${input.rangeLabel ? esc(` ${t("gc.kpiRange")} · ${input.rangeLabel}`) : ""}</h2><div class="table-scroll"><table class="gc-files gc-compare-table"><thead><tr><th>${esc(t("gc.groups"))}</th>${cols
    .map(([k]) => `<th class="num">${esc(t(k))}</th>`)
    .join("")}</tr></thead><tbody>${rows}</tbody></table></div></section>`;
  const blocks = input.groups
    .map((x, i) => {
      const g = x.group;
      const charts = x.series
        ? `<h3 class="gc-multiple-title">${esc(t("gc.chart.pauses"))}</h3>${pauseSvg(x.series, axis, g.wall_offset_ms, t("gc.chart.pauses"))}<h3 class="gc-multiple-title">${esc(t("gc.chart.heap"))}</h3>${heapSvg(x.series, axis, g.wall_offset_ms, t("gc.chart.heap"))}`
        : "";
      return `<section class="panel gc-multiple" id="gc-compare-${i}"><h2>${esc(groupLabel(g, names))} <span class="gc-compare-collector">· ${esc(collectorLabel(g.collector))}</span></h2>${kpiTiles(g.stats, t)}${findingsPanel(g.findings, g.event_count, t, `gc-compare-findings-${i}`)}${charts}</section>`;
    })
    .join("\n");
  return `${table}\n<div class="gc-compare-blocks">${blocks}</div>`;
}

export function buildGcReportHtml(input: GcExportInput): string {
  const { t, locale, names } = input;
  const single = input.groups.length === 1;
  const title = single ? groupLabel(input.groups[0].group, names) : t("gc.compare.chip", { count: input.groups.length });
  const body = single ? singleGroupBody(input.groups[0], input) : { nav: ["gc-overview"] as ReportSectionId[], html: compareBody(input) };
  const axisNote = input.axis === "wall" ? t("gc.axis.wall") : t("gc.axis.uptime");
  return `<!doctype html>
<html lang="${htmlLangFor(locale)}">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>jblock — ${esc(t("gc.export.title"))} — ${esc(title)}</title>
<style>${appCss}
.gc-svg { width: 100%; height: auto; display: block; }
.gc-compare-blocks { display: flex; flex-direction: column; gap: var(--space-3); }
</style>
</head>
<body>
<div class="app report">
  <header class="app-header">
    <h1><span class="logo">jblock</span> ${esc(t("gc.export.title"))}</h1>
    <p class="tagline">${esc(title)} · ${esc(t("gc.axis.label"))}: ${esc(axisNote)} · ${esc(t("gc.export.generated", { time: formatWall(Date.now()) }))}</p>
  </header>
  <div class="results-shell">
  ${navHtml(body.nav, t)}
  <div class="results">
  ${body.html}
  </div>
  </div>
</div>
</body>
</html>`;
}

/** Download the GC report as a standalone HTML file. */
export function exportGcHtml(input: GcExportInput): void {
  const html = buildGcReportHtml(input);
  const base = input.groups.length === 1 ? groupLabel(input.groups[0].group, input.names) : "compare";
  const url = URL.createObjectURL(new Blob([html], { type: "text/html" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = `jblock-gc-report-${base.replace(/[^\w.-]+/g, "_")}.html`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
