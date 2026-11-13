import { useCallback, useEffect, useMemo, useState } from "react";
import { gcSeries, gcStats } from "./analyzer";
import { worstRow, type TimeAxis } from "./gcCharts";
import { collectorLabel, formatMs, formatPct, formatRate, formatWall } from "./gcFormat";
import { HeapChart, PauseTimeline } from "./GcTimeCharts";
import type { GcReport, GcSeries, GcStats, GroupReport } from "./gcTypes";
import { exportGcHtml } from "./gcExport";
import { useI18n, type MessageKey } from "./i18n";

/** More JVMs than this must be picked explicitly (one small multiple each). */
const MAX_COMPARE = 8;
const MAX_CAUSE_ROWS = 12;

interface Props {
  report: GcReport;
  names: string[];
}

interface Column {
  key: MessageKey;
  value: (s: GcStats) => number | null;
  format: (v: number | null) => string;
  /** Which direction is worse. */
  worse: "high" | "low";
}

const fullCount = (s: GcStats) => s.kinds.find((k) => k.kind === "full")?.count ?? 0;

const COLUMNS: Column[] = [
  { key: "gc.kpi.throughput", value: (s) => s.throughput_pct, format: formatPct, worse: "low" },
  { key: "gc.kpi.p99", value: (s) => s.pause?.p99 ?? null, format: formatMs, worse: "high" },
  { key: "gc.kpi.max", value: (s) => s.pause?.max ?? null, format: formatMs, worse: "high" },
  { key: "gc.kpi.totalPause", value: (s) => s.total_pause_ms, format: formatMs, worse: "high" },
  { key: "gc.kpi.full", value: (s) => fullCount(s), format: (v) => (v == null ? "—" : String(v)), worse: "high" },
  { key: "gc.kpi.alloc", value: (s) => s.allocation?.avg_mb_per_s ?? null, format: formatRate, worse: "high" },
  { key: "gc.kpi.heapPeak", value: (s) => s.heap_after_peak_pct, format: formatPct, worse: "high" },
];

function groupLabel(g: GroupReport, names: string[]): string {
  const first = names[g.files[0]] ?? g.name;
  return g.files.length > 1 ? `${first} (+${g.files.length - 1})` : first;
}

export default function GcCompare({ report, names }: Props) {
  const { t, locale } = useI18n();
  const all = report.groups;
  const [picked, setPicked] = useState<number[]>(() => all.slice(0, MAX_COMPARE).map((g) => g.index));
  const groups = useMemo(() => all.filter((g) => picked.includes(g.index)), [all, picked]);
  const wallOk = groups.length > 0 && groups.every((g) => g.has_wall && g.wall_offset_ms != null);
  const [axisPref, setAxisPref] = useState<TimeAxis>("uptime");
  const axis: TimeAxis = axisPref === "wall" && wallOk ? "wall" : "uptime";
  const [series, setSeries] = useState<Record<number, GcSeries>>({});
  const [range, setRange] = useState<[number, number] | null>(null);
  const [rangeStats, setRangeStats] = useState<Record<number, GcStats> | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSeries({});
    void Promise.all(groups.filter((g) => g.event_count > 0).map(async (g) => [g.index, await gcSeries(g.index)] as const)).then(
      (pairs) => {
        if (!cancelled) setSeries(Object.fromEntries(pairs));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [groups]);

  // A brushed range is in chart x units (s); convert to each group's own time axis.
  const toGroupMs = useCallback(
    (g: GroupReport, x: number) => (axis === "wall" && g.wall_offset_ms != null ? x * 1000 - g.wall_offset_ms : x * 1000),
    [axis],
  );
  useEffect(() => {
    if (!range) {
      setRangeStats(null);
      return;
    }
    let cancelled = false;
    void Promise.all(
      groups.map(async (g) => [g.index, await gcStats(g.index, toGroupMs(g, range[0]), toGroupMs(g, range[1]))] as const),
    ).then((pairs) => {
      if (!cancelled) setRangeStats(Object.fromEntries(pairs));
    });
    return () => {
      cancelled = true;
    };
  }, [range, groups, toGroupMs]);

  // Shared x domain so the small multiples line up.
  const domain = useMemo<[number, number] | null>(() => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const g of groups) {
      if (g.event_count === 0) continue;
      const off = axis === "wall" ? (g.wall_offset_ms ?? 0) : 0;
      lo = Math.min(lo, (g.stats.start_ms + off) / 1000);
      hi = Math.max(hi, (g.stats.end_ms + off) / 1000);
    }
    return Number.isFinite(lo) && hi > lo ? [lo, hi] : null;
  }, [groups, axis]);
  const xRange = range ?? domain;

  const statsOf = (g: GroupReport) => rangeStats?.[g.index] ?? g.stats;
  const onSelect = useCallback((x0: number, x1: number) => setRange([Math.min(x0, x1), Math.max(x0, x1)]), []);
  const onReset = useCallback(() => setRange(null), []);
  const label = (x: number) => (axis === "wall" ? formatWall(x * 1000) : formatMs(x * 1000));

  const causes = useMemo(() => {
    const totals = new Map<string, number>();
    for (const g of groups) for (const c of statsOf(g).causes) totals.set(c.cause, (totals.get(c.cause) ?? 0) + c.total_pause_ms);
    return [...totals.entries()].sort((a, b) => b[1] - a[1]).slice(0, MAX_CAUSE_ROWS).map(([c]) => c);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [groups, rangeStats]);

  return (
    <div className="gc-compare results" data-testid="gc-compare">
      {all.length > MAX_COMPARE && (
        <section className="panel" data-testid="gc-compare-picker">
          <p className="import-warning">{t("gc.compare.tooMany", { max: MAX_COMPARE, count: all.length })}</p>
          <div className="gc-compare-picker">
            {all.map((g) => {
              const on = picked.includes(g.index);
              return (
                <label key={g.index} className="gc-toggle">
                  <input
                    type="checkbox"
                    checked={on}
                    disabled={!on && picked.length >= MAX_COMPARE}
                    onChange={() => setPicked((p) => (on ? p.filter((i) => i !== g.index) : [...p, g.index]))}
                  />
                  {groupLabel(g, names)}
                </label>
              );
            })}
          </div>
        </section>
      )}

      <div className="gc-filters">
        {wallOk && (
          <div className="segmented" role="radiogroup" aria-label={t("gc.axis.label")}>
            <span className="dump-series-label">{t("gc.axis.label")}</span>
            {(["uptime", "wall"] as const).map((a) => (
              <button
                key={a}
                type="button"
                role="radio"
                aria-checked={axis === a}
                className={`segmented-option${axis === a ? " active" : ""}`}
                onClick={() => {
                  setAxisPref(a);
                  setRange(null);
                }}
              >
                {t(a === "uptime" ? "gc.axis.uptime" : "gc.axis.wall")}
              </button>
            ))}
          </div>
        )}
        {range && (
          <span className="gc-range-chip" data-testid="gc-range-chip">
            {t("gc.range.chip", { from: label(range[0]), to: label(range[1]) })}
            <button type="button" aria-label={t("gc.range.reset")} onClick={onReset} data-testid="gc-range-reset">
              ×
            </button>
          </span>
        )}
        <span className="gc-hint">{t("gc.compare.hint")}</span>
        <button
          type="button"
          className="btn gc-export-btn"
          data-testid="gc-export"
          onClick={() =>
            exportGcHtml({
              report,
              names,
              groups: groups.map((g) => ({ group: g, series: series[g.index] ?? null, rangeStats: rangeStats?.[g.index] ?? null })),
              axis,
              rangeLabel: range ? `${label(range[0])} – ${label(range[1])}` : null,
              t,
              locale,
            })
          }
        >
          {t("app.exportHtml")}
        </button>
      </div>

      <section className="panel" data-testid="gc-compare-kpis">
        <h2>
          {t("gc.compare.kpis")}
          {rangeStats ? ` ${t("gc.kpiRange")}` : ""}
        </h2>
        <div className="table-scroll">
          <table className="gc-files gc-compare-table">
            <thead>
              <tr>
                <th>{t("gc.groups")}</th>
                {COLUMNS.map((c) => (
                  <th key={c.key} className="num">
                    {t(c.key)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {groups.map((g, row) => (
                <tr key={g.index} data-testid={`gc-compare-row-${g.index}`}>
                  <td>
                    <span className="mono">{groupLabel(g, names)}</span>
                    <span className="gc-compare-collector"> · {collectorLabel(g.collector)}</span>
                  </td>
                  {COLUMNS.map((c) => {
                    const values = groups.map((x) => c.value(statsOf(x)));
                    const worst = worstRow(values, c.worse) === row;
                    return (
                      <td key={c.key} className={`num${worst ? " gc-worst" : ""}`} data-worst={worst || undefined}>
                        {c.format(values[row])}
                        {worst && <span className="gc-worst-badge">{t("gc.compare.worst")}</span>}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="panel" data-testid="gc-compare-causes">
        <h2>{t("gc.chart.causes")}</h2>
        <div className="table-scroll">
          <table className="gc-files">
            <thead>
              <tr>
                <th>{t("gc.events.cause")}</th>
                {groups.map((g) => (
                  <th key={g.index} className="num">
                    {groupLabel(g, names)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {causes.map((cause) => (
                <tr key={cause}>
                  <td>{cause}</td>
                  {groups.map((g) => {
                    const c = statsOf(g).causes.find((x) => x.cause === cause);
                    return (
                      <td key={g.index} className="num mono">
                        {c ? `${formatMs(c.total_pause_ms)} · ${c.count}×` : "—"}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <div className="gc-multiples">
        {groups.map((g) => (
          <section key={g.index} className="panel gc-multiple" data-testid={`gc-compare-chart-${g.index}`}>
            <h2>
              {groupLabel(g, names)} <span className="gc-compare-collector">· {collectorLabel(g.collector)}</span>
            </h2>
            {g.event_count === 0 ? (
              <p className="empty">{t("gc.noEvents")}</p>
            ) : series[g.index] ? (
              <>
                <h3 className="gc-multiple-title">{t("gc.chart.pauses")}</h3>
                <PauseTimeline
                  series={series[g.index]}
                  axis={axis}
                  wallOffsetMs={g.wall_offset_ms}
                  xRange={xRange}
                  onSelect={onSelect}
                  onReset={onReset}
                  syncKey="gc-compare"
                  logScale
                />
                <h3 className="gc-multiple-title">{t("gc.chart.heap")}</h3>
                <HeapChart
                  series={series[g.index]}
                  axis={axis}
                  wallOffsetMs={g.wall_offset_ms}
                  xRange={xRange}
                  onSelect={onSelect}
                  onReset={onReset}
                  syncKey="gc-compare"
                />
              </>
            ) : null}
          </section>
        ))}
      </div>
    </div>
  );
}
