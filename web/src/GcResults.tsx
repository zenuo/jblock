import { useCallback, useEffect, useMemo, useState } from "react";
import { gcSeries, gcStats } from "./analyzer";
import { fromX, lowerBound, preferLogScale, toX, type TimeAxis } from "./gcCharts";
import { exportGcHtml } from "./gcExport";
import { findingText, locateRange, type Translate } from "./gcFindings";
import {
  collectorLabel,
  formatKb,
  formatMs,
  formatPct,
  formatRate,
  formatWall,
} from "./gcFormat";
import GcCompare from "./GcCompare";
import { CauseBars, MetaspaceChart, PauseHistogram, RateChart } from "./GcDistCharts";
import GcEventTable from "./GcEventTable";
import { DEFAULT_TABLE_QUERY, type TableQuery } from "./gcTable";
import { HeapChart, PauseTimeline } from "./GcTimeCharts";
import type {
  FileFingerprint,
  GcReport,
  GcSeries,
  GcStats,
  GroupNote,
  GroupReport,
  GroupingMode,
} from "./gcTypes";
import { useI18n, type MessageKey, type TranslateFn } from "./i18n";
import ReportNav from "./ReportNav";
import {
  gcReportSections,
  parseSectionId,
  readNavCollapsed,
  sectionDomId,
  storeNavCollapsed,
  type ReportSection,
  type ReportSectionId,
} from "./reportNav";

interface Props {
  report: GcReport;
  /** Display names of the GC files, indexed like `report.files`. */
  names: string[];
  onRegroup: (mode: GroupingMode) => void;
  busy?: boolean;
}

const MODES: { mode: GroupingMode; key: MessageKey }[] = [
  { mode: "auto", key: "gc.groupingAuto" },
  { mode: "merge", key: "gc.groupingMerge" },
  { mode: "split", key: "gc.groupingSplit" },
];

function noteText(note: GroupNote, names: string[], t: TranslateFn): string {
  const file = names[note.file_index] ?? `#${note.file_index}`;
  const [, value = ""] = note.detail.split("=");
  switch (note.kind) {
    case "gap":
      return t("gc.note.gap", { file, missing: value });
    case "overlap":
      return t("gc.note.overlap", { file, ms: formatMs(Number(value)) });
    case "duplicate":
      return t("gc.note.duplicate", { file, orig: names[Number(value)] ?? value });
    case "missing-start":
      return t("gc.note.missingStart", { file, id: value });
  }
}

function fullCount(stats: GcStats): number {
  return stats.kinds.find((k) => k.kind === "full")?.count ?? 0;
}

export function KpiTiles({ stats, ranged }: { stats: GcStats; ranged: boolean }) {
  const { t } = useI18n();
  const full = fullCount(stats);
  const tiles: { key: MessageKey; value: string; tone?: "danger" }[] = [
    { key: "gc.kpi.throughput", value: formatPct(stats.throughput_pct) },
    { key: "gc.kpi.p99", value: formatMs(stats.pause?.p99) },
    { key: "gc.kpi.max", value: formatMs(stats.pause?.max) },
    { key: "gc.kpi.totalPause", value: formatMs(stats.total_pause_ms) },
    { key: "gc.kpi.pauses", value: String(stats.pause_count) },
    { key: "gc.kpi.full", value: String(full), tone: full > 0 ? "danger" : undefined },
    { key: "gc.kpi.span", value: formatMs(stats.span_ms) },
    { key: "gc.kpi.alloc", value: formatRate(stats.allocation?.avg_mb_per_s) },
    { key: "gc.kpi.heapPeak", value: formatPct(stats.heap_after_peak_pct) },
  ];
  return (
    <section
      className={`gc-kpis${ranged ? " is-ranged" : ""}`}
      data-testid="gc-kpis"
      data-ranged={ranged ? "true" : "false"}
      aria-label={ranged ? `${t("gc.kpiLabel")} ${t("gc.kpiRange")}` : t("gc.kpiLabel")}
    >
      {tiles.map((tile) => (
        <div
          key={tile.key}
          className={`stat${tile.tone ? ` ${tile.tone}` : ""}`}
          data-testid={`gc-kpi-${tile.key.slice("gc.kpi.".length)}`}
        >
          <span className="stat-value">{tile.value}</span>
          <span className="stat-label">{t(tile.key)}</span>
        </div>
      ))}
    </section>
  );
}

const SEV_MARK: Record<string, string> = { critical: "⛔", warning: "⚠️", info: "ℹ️" };

export function GcFindingsPanel({
  group,
  onLocate,
}: {
  group: GroupReport;
  onLocate?: (range: [number, number]) => void;
}) {
  const { t } = useI18n();
  return (
    <section className="panel findings" id={sectionDomId("gc-findings")} data-testid="gc-findings">
      <h2>{t("gc.findings.title")}</h2>
      {group.findings.length === 0 ? (
        <div className="finding finding-ok" data-testid="gc-findings-ok">
          <div className="finding-row">
            <strong>
              <span className="finding-ok-mark" aria-hidden="true">
                ✅
              </span>
              {t("gc.findings.okTitle")}
            </strong>
          </div>
          <span className="mono">{t("gc.findings.okDetail", { count: group.event_count })}</span>
        </div>
      ) : (
        <ul className="findings-list">
          {group.findings.map((f) => {
            const text = findingText(f, t as Translate);
            const range = locateRange(f);
            return (
              <li key={f.rule} className={`finding finding-${f.severity}`} data-testid={`gc-finding-${f.rule}`} data-severity={f.severity}>
                <div className="finding-row">
                  <strong>
                    <span className="gc-sev" aria-hidden="true">
                      {SEV_MARK[f.severity]}
                    </span>
                    <span className={`gc-sev-label gc-sev-${f.severity}`}>{t(`gc.sev.${f.severity}` as MessageKey)}</span>
                    {text.title}
                  </strong>
                  <span className="gc-finding-actions">
                    <span className="mono gc-rule-id">{f.rule}</span>
                    {range && onLocate && (
                      <button
                        type="button"
                        className="btn finding-legend-btn"
                        data-testid={`gc-locate-${f.rule}`}
                        onClick={() => onLocate(range)}
                      >
                        {t("gc.findings.locate")}
                      </button>
                    )}
                  </span>
                </div>
                <p className="gc-finding-body">{text.body}</p>
                <p className="gc-finding-fix">
                  <span className="gc-finding-fix-label">{t("gc.findings.fixLabel")}</span>
                  {text.fix}
                </p>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

export function JvmPanel({ group }: { group: GroupReport }) {
  const { t } = useI18n();
  const formatKey: MessageKey =
    group.format === "unified"
      ? "gc.format.unified"
      : group.format === "legacy"
        ? "gc.format.legacy"
        : "gc.format.unknown";
  const rows: [MessageKey, string | null][] = [
    ["gc.jvm.collector", collectorLabel(group.collector)],
    ["gc.jvm.format", t(formatKey)],
    ["gc.jvm.version", group.jvm.version],
    ["gc.jvm.heapMax", group.jvm.heap_max_kb != null ? formatKb(group.jvm.heap_max_kb) : null],
    ["gc.jvm.heapInitial", group.jvm.heap_initial_kb != null ? formatKb(group.jvm.heap_initial_kb) : null],
    ["gc.jvm.regionSize", group.jvm.region_size_kb != null ? formatKb(group.jvm.region_size_kb) : null],
    ["gc.jvm.commandLine", group.jvm.command_line],
  ];
  return (
    <section className="panel gc-jvm" data-testid="gc-jvm">
      <h2>{t("gc.jvmInfo")}</h2>
      <dl className="gc-facts">
        {rows
          .filter(([, v]) => v)
          .map(([k, v]) => (
            <div key={k} className="gc-fact">
              <dt>{t(k)}</dt>
              <dd className={k === "gc.jvm.commandLine" ? "mono" : undefined}>{v}</dd>
            </div>
          ))}
      </dl>
    </section>
  );
}

export function DiagnosticsPanel({
  group,
  files,
  names,
}: {
  group: GroupReport;
  files: FileFingerprint[];
  names: string[];
}) {
  const { t } = useI18n();
  const withSamples = group.files.filter((i) => files[i].unparsed_count > 0);
  return (
    <section className="panel gc-diagnostics" data-testid="gc-diagnostics">
      <h2>{t("gc.diagnostics")}</h2>
      {group.notes.length > 0 && (
        <ul className="gc-notes" data-testid="gc-notes">
          {group.notes.map((n, i) => (
            <li key={i} data-note={n.kind}>
              {noteText(n, names, t)}
            </li>
          ))}
        </ul>
      )}
      <div className="table-scroll">
        <table className="gc-files">
          <thead>
            <tr>
              <th>{t("gc.diag.file")}</th>
              <th>{t("gc.diag.format")}</th>
              <th>{t("gc.diag.collector")}</th>
              <th className="num">{t("gc.diag.events")}</th>
              <th>{t("gc.diag.gcIds")}</th>
              <th>{t("gc.diag.timeRange")}</th>
              <th className="num">{t("gc.diag.unparsed")}</th>
            </tr>
          </thead>
          <tbody>
            {group.files.map((i) => {
              const f = files[i];
              return (
                <tr key={i}>
                  <td className="mono">{names[i]}</td>
                  <td>{f.format}</td>
                  <td>{collectorLabel(f.collector)}</td>
                  <td className="num">{f.event_count}</td>
                  <td className="mono">
                    {f.first_gc_id != null ? `${f.first_gc_id}–${f.last_gc_id}` : "—"}
                  </td>
                  <td className="mono">
                    {f.first_uptime_ms != null
                      ? `${formatMs(f.first_uptime_ms)} – ${formatMs(f.last_uptime_ms)}`
                      : "—"}
                  </td>
                  <td className="num">{f.unparsed_count}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {withSamples.map((i) => (
        <details key={i} className="gc-unparsed">
          <summary>
            {t("gc.diag.unparsedSamples", {
              name: names[i],
              count: files[i].unparsed_samples.length,
            })}
          </summary>
          <pre className="mono">
            {files[i].unparsed_samples.map(([line, text]) => `${line}: ${text}`).join("\n")}
          </pre>
        </details>
      ))}
    </section>
  );
}

/** Highlight the nav entry of the section most in view (same behavior as the thread report). */
function useActiveSection(sections: ReportSection[], dep: unknown): [ReportSectionId | null, (id: ReportSectionId) => void] {
  const [active, setActive] = useState<ReportSectionId | null>(sections[0]?.id ?? null);
  useEffect(() => {
    const els = sections
      .map((s) => document.getElementById(sectionDomId(s.id)))
      .filter((el): el is HTMLElement => el !== null);
    if (els.length === 0) return;
    const ratios = new Map<string, number>();
    const obs = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) ratios.set(entry.target.id, entry.isIntersecting ? entry.intersectionRatio : 0);
        let bestId: string | null = null;
        let best = 0;
        for (const [id, ratio] of ratios) {
          if (ratio > best) {
            best = ratio;
            bestId = id;
          }
        }
        const parsed = bestId ? parseSectionId(bestId) : null;
        if (parsed) setActive(parsed);
      },
      { rootMargin: "-12% 0px -55% 0px", threshold: [0, 0.15, 0.35, 0.6, 1] },
    );
    for (const el of els) obs.observe(el);
    return () => obs.disconnect();
  }, [sections, dep]);
  return [active, setActive];
}

/** Group-chip selection meaning "compare all JVMs" (feat-078). */
const COMPARE = -1;

export default function GcResults({ report, names, onRegroup, busy }: Props) {
  const { t, locale } = useI18n();
  const [selected, setSelected] = useState(0);
  const comparing = selected === COMPARE && report.groups.length > 1;
  const groupIndex = Math.min(Math.max(selected, 0), Math.max(report.groups.length - 1, 0));
  const group = report.groups[groupIndex];

  const [series, setSeries] = useState<GcSeries | null>(null);
  const [range, setRange] = useState<[number, number] | null>(null);
  const [rangeStats, setRangeStats] = useState<GcStats | null>(null);
  const [axis, setAxis] = useState<TimeAxis>("uptime");
  const [logScale, setLogScale] = useState<boolean | null>(null);
  const [navCollapsed, setNavCollapsed] = useState(() => readNavCollapsed());
  const [tableQuery, setTableQuery] = useState<TableQuery>(DEFAULT_TABLE_QUERY);
  const [selectedRow, setSelectedRow] = useState<number | null>(null);
  const [scrollTo, setScrollTo] = useState<{ position: number; nonce: number } | null>(null);

  // Load chart columns for the selected group; a new group / regroup starts unzoomed.
  useEffect(() => {
    let cancelled = false;
    setSeries(null);
    setRange(null);
    setRangeStats(null);
    setLogScale(null);
    setSelectedRow(null);
    setTableQuery(DEFAULT_TABLE_QUERY);
    if (!group || group.event_count === 0) return;
    void gcSeries(groupIndex).then((cols) => {
      if (!cancelled) setSeries(cols);
    });
    return () => {
      cancelled = true;
    };
  }, [report, groupIndex, group]);

  // Range KPIs come from the same Rust stats engine (worker round trip).
  useEffect(() => {
    if (!range) return;
    let cancelled = false;
    void gcStats(groupIndex, range[0], range[1]).then((stats) => {
      if (!cancelled) setRangeStats(stats);
    });
    return () => {
      cancelled = true;
    };
  }, [range, groupIndex]);

  // Only a live range shows ranged KPIs; while a new range loads the previous frame stays.
  const shownStats = range && rangeStats ? rangeStats : group?.stats;
  const wallOffset = group?.wall_offset_ms ?? null;
  const effectiveAxis: TimeAxis = axis === "wall" && group?.has_wall && wallOffset != null ? "wall" : "uptime";
  const useLog = logScale ?? (series ? preferLogScale(series.duration_ms) : false);
  const xRange = useMemo<[number, number] | null>(
    () => (range ? [toX(range[0], effectiveAxis, wallOffset), toX(range[1], effectiveAxis, wallOffset)] : null),
    [range, effectiveAxis, wallOffset],
  );
  const onSelect = useCallback(
    (x0: number, x1: number) => {
      const a = fromX(Math.min(x0, x1), effectiveAxis, wallOffset);
      const b = fromX(Math.max(x0, x1), effectiveAxis, wallOffset);
      setRange([a, b]);
    },
    [effectiveAxis, wallOffset],
  );
  const onReset = useCallback(() => setRange(null), []);
  // Chart click: show that event in the table (time order, no filters, current range).
  const onPickRow = useCallback(
    (row: number) => {
      if (!series) return;
      setSelectedRow(row);
      setTableQuery(DEFAULT_TABLE_QUERY);
      const first = range ? lowerBound(series.time_ms, range[0]) : 0;
      setScrollTo({ position: Math.max(row - first, 0), nonce: Date.now() });
    },
    [series, range],
  );
  const onLocate = useCallback((r: [number, number]) => {
    setRange(r);
    document.getElementById(sectionDomId("gc-pauses"))?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  const sections = useMemo(() => gcReportSections({ hasEvents: (group?.event_count ?? 0) > 0 }), [group]);
  const [activeSection, setActiveSection] = useActiveSection(sections, group);

  const chips = useMemo(
    () =>
      report.groups.map((g) => ({
        index: g.index,
        label: t("gc.groupChip", {
          name: g.files.length > 1 ? `${g.name} (+${g.files.length - 1})` : g.name,
          collector: collectorLabel(g.collector),
          span: formatMs(g.stats.span_ms),
        }),
      })),
    [report.groups, t],
  );

  const rangeLabel = (ms: number) =>
    effectiveAxis === "wall" && wallOffset != null ? formatWall(ms + wallOffset) : formatMs(ms);

  const jumpToSection = (domId: string) => {
    const parsed = parseSectionId(domId);
    if (parsed) setActiveSection(parsed);
    document.getElementById(domId)?.scrollIntoView({ behavior: "smooth", block: "start" });
    if (typeof history !== "undefined" && history.replaceState) history.replaceState(null, "", `#${domId}`);
  };

  const toggleNav = () =>
    setNavCollapsed((prev) => {
      storeNavCollapsed(!prev);
      return !prev;
    });

  const syncKey = `gc-${groupIndex}`;

  return (
    <div className="gc-results" data-testid="gc-results">
      <section className="dump-series gc-groups" data-testid="gc-groups" aria-label={t("gc.groups")}>
        <span className="dump-series-label">{t("gc.groups")}</span>
        <div className="dump-series-chips">
          {chips.map((c) => (
            <button
              key={c.index}
              type="button"
              className={`dump-chip${!comparing && c.index === groupIndex ? " active" : ""}`}
              data-testid={`gc-group-chip-${c.index}`}
              onClick={() => setSelected(c.index)}
            >
              {c.label}
            </button>
          ))}
          {report.groups.length > 1 && (
            <button
              type="button"
              className={`dump-chip gc-compare-chip${comparing ? " active" : ""}`}
              data-testid="gc-compare-chip"
              onClick={() => setSelected(COMPARE)}
            >
              {t("gc.compare.chip", { count: report.groups.length })}
            </button>
          )}
        </div>
        <div className="segmented" role="radiogroup" aria-label={t("gc.grouping")} title={t("gc.groupingHint")}>
          <span className="dump-series-label">{t("gc.grouping")}</span>
          {MODES.map(({ mode, key }) => (
            <button
              key={mode}
              type="button"
              role="radio"
              aria-checked={report.mode === mode}
              className={`segmented-option${report.mode === mode ? " active" : ""}`}
              data-testid={`gc-mode-${mode}`}
              disabled={busy}
              onClick={() => {
                if (report.mode !== mode) {
                  setSelected(0);
                  onRegroup(mode);
                }
              }}
            >
              {t(key)}
            </button>
          ))}
        </div>
      </section>

      {comparing && <GcCompare key={report.mode} report={report} names={names} />}

      {group && !comparing && (
        <div className={`results-shell${navCollapsed ? " is-nav-collapsed" : ""}`} data-testid="gc-results-shell">
          <ReportNav
            sections={sections}
            collapsed={navCollapsed}
            onToggle={toggleNav}
            activeId={activeSection}
            onNavigate={jumpToSection}
          />
          <main className="results">
            {group.event_count > 0 && (
              <div className="gc-filters" data-testid="gc-filters">
                {group.has_wall && wallOffset != null && (
                  <div className="segmented" role="radiogroup" aria-label={t("gc.axis.label")}>
                    <span className="dump-series-label">{t("gc.axis.label")}</span>
                    {(["uptime", "wall"] as const).map((a) => (
                      <button
                        key={a}
                        type="button"
                        role="radio"
                        aria-checked={effectiveAxis === a}
                        className={`segmented-option${effectiveAxis === a ? " active" : ""}`}
                        data-testid={`gc-axis-${a}`}
                        onClick={() => setAxis(a)}
                      >
                        {t(a === "uptime" ? "gc.axis.uptime" : "gc.axis.wall")}
                      </button>
                    ))}
                  </div>
                )}
                {range && (
                  <span className="gc-range-chip" data-testid="gc-range-chip">
                    {t("gc.range.chip", { from: rangeLabel(range[0]), to: rangeLabel(range[1]) })}
                    <button type="button" aria-label={t("gc.range.reset")} title={t("gc.range.reset")} data-testid="gc-range-reset" onClick={onReset}>
                      ×
                    </button>
                  </span>
                )}
                <span className="gc-hint">{t("gc.chart.hint")}</span>
                <button
                  type="button"
                  className="btn gc-export-btn"
                  data-testid="gc-export"
                  disabled={!series}
                  onClick={() =>
                    exportGcHtml({
                      report,
                      names,
                      groups: [{ group, series, rangeStats: range ? rangeStats : null }],
                      axis: effectiveAxis,
                      rangeLabel: range ? `${rangeLabel(range[0])} – ${rangeLabel(range[1])}` : null,
                      t,
                      locale,
                    })
                  }
                >
                  {t("app.exportHtml")}
                </button>
              </div>
            )}

            <section id={sectionDomId("gc-overview")} className="gc-section" data-testid="gc-overview">
              {group.event_count === 0 ? (
                <p className="empty" data-testid="gc-no-events">
                  {t("gc.noEvents")}
                </p>
              ) : (
                <>
                  {group.missing_timestamps && <p className="empty">{t("gc.noTimestamps")}</p>}
                  {range != null && rangeStats != null && (
                    <p className="gc-kpi-caption" data-testid="gc-kpi-caption">
                      {t("gc.kpiLabel")} {t("gc.kpiRange")}
                    </p>
                  )}
                  <KpiTiles stats={shownStats} ranged={range != null && rangeStats != null} />
                </>
              )}
            </section>

            <GcFindingsPanel group={group} onLocate={group.event_count > 0 ? onLocate : undefined} />

            {group.event_count > 0 && (
              <>
                <section className="panel" id={sectionDomId("gc-pauses")} data-testid="gc-pauses">
                  <div className="gc-panel-head">
                    <h2>{t("gc.chart.pauses")}</h2>
                    <label className="gc-toggle">
                      <input
                        type="checkbox"
                        checked={useLog}
                        onChange={(e) => setLogScale(e.target.checked)}
                        data-testid="gc-log-scale"
                      />
                      {t("gc.chart.logScale")}
                    </label>
                  </div>
                  {series && (
                    <PauseTimeline
                      series={series}
                      axis={effectiveAxis}
                      wallOffsetMs={wallOffset}
                      xRange={xRange}
                      onSelect={onSelect}
                      onReset={onReset}
                      syncKey={syncKey}
                      logScale={useLog}
                      markRow={selectedRow}
                      onPickRow={onPickRow}
                    />
                  )}
                </section>
                <section className="panel" id={sectionDomId("gc-heap")} data-testid="gc-heap">
                  <h2>{t("gc.chart.heap")}</h2>
                  {series && (
                    <HeapChart
                      series={series}
                      axis={effectiveAxis}
                      wallOffsetMs={wallOffset}
                      xRange={xRange}
                      onSelect={onSelect}
                      onReset={onReset}
                      syncKey={syncKey}
                      markRow={selectedRow}
                      onPickRow={onPickRow}
                    />
                  )}
                </section>
                {shownStats && (
                  <div className="gc-dist-grid" id={sectionDomId("gc-distribution")}>
                    <section className="panel" data-testid="gc-distribution">
                      <h2>{t("gc.chart.histogram")}</h2>
                      <PauseHistogram stats={shownStats} />
                    </section>
                    <section className="panel" data-testid="gc-causes-panel">
                      <h2>{t("gc.chart.causes")}</h2>
                      <CauseBars causes={shownStats.causes} />
                    </section>
                  </div>
                )}
                <section className="panel" id={sectionDomId("gc-rates")} data-testid="gc-rates">
                  <h2>{t("gc.chart.rates")}</h2>
                  {series && (
                    <RateChart
                      series={series}
                      axis={effectiveAxis}
                      wallOffsetMs={wallOffset}
                      xRange={xRange}
                      onSelect={onSelect}
                      onReset={onReset}
                      syncKey={syncKey}
                      markRow={selectedRow}
                    />
                  )}
                </section>
                <section className="panel" id={sectionDomId("gc-metaspace")} data-testid="gc-metaspace">
                  <h2>{t("gc.chart.metaspace")}</h2>
                  {series && (
                    <MetaspaceChart
                      series={series}
                      axis={effectiveAxis}
                      wallOffsetMs={wallOffset}
                      xRange={xRange}
                      onSelect={onSelect}
                      onReset={onReset}
                      syncKey={syncKey}
                      markRow={selectedRow}
                    />
                  )}
                </section>
              </>
            )}
            {group.event_count > 0 && (
              <section className="panel" id={sectionDomId("gc-events")} data-testid="gc-events-panel">
                <h2>{t("gc.events.title")}</h2>
                <GcEventTable
                  group={groupIndex}
                  version={report}
                  range={range}
                  causes={group.stats.causes}
                  kinds={group.stats.kinds.map((k) => k.kind)}
                  axis={effectiveAxis}
                  query={tableQuery}
                  onQuery={setTableQuery}
                  selectedRow={selectedRow}
                  onSelectRow={setSelectedRow}
                  scrollTo={scrollTo}
                  names={names}
                />
              </section>
            )}
            <div id={sectionDomId("gc-jvm")}>
              <JvmPanel group={group} />
            </div>
            <div id={sectionDomId("gc-diagnostics")}>
              <DiagnosticsPanel group={group} files={report.files} names={names} />
            </div>
          </main>
        </div>
      )}
    </div>
  );
}
