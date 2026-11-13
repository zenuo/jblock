import { useCallback, useMemo, useState } from "react";
import type uPlot from "uplot";
import GcChart, { type TipRow } from "./GcChart";
import {
  CHROME,
  HEAP_COLORS,
  RATE_COLORS,
  baseAxis,
  bucketLabel,
  buildMetaspaceData,
  buildRateData,
  toX,
  type TimeAxis,
} from "./gcCharts";
import { formatKb, formatMs, formatPct, formatRate, formatWall } from "./gcFormat";
import type { CauseStat, GcSeries, GcStats } from "./gcTypes";
import { useI18n } from "./i18n";

/** Magnitude-only bars wear secondary ink, never an entity color. */
const BAR_COLOR = CHROME.secondary;
const MAX_CAUSES = 10;

function Legend({ items }: { items: { color: string; label: string }[] }) {
  return (
    <ul className="gc-legend">
      {items.map((it) => (
        <li key={it.label}>
          <span className="gc-legend-key line" style={{ background: it.color }} />
          {it.label}
        </li>
      ))}
    </ul>
  );
}

export function PauseHistogram({ stats }: { stats: GcStats }) {
  const { t } = useI18n();
  const [hover, setHover] = useState<number | null>(null);
  const buckets = stats.histogram;
  const total = buckets.reduce((a, b) => a + b.count, 0);
  if (buckets.length === 0 || total === 0) return <p className="empty">{t("gc.noEvents")}</p>;
  const max = Math.max(...buckets.map((b) => b.count));
  const W = 560;
  const H = 200;
  const pad = { l: 8, r: 8, t: 22, b: 40 };
  const slot = (W - pad.l - pad.r) / buckets.length;
  const bw = Math.min(24, slot * 0.7);
  const y = (v: number) => pad.t + (H - pad.t - pad.b) * (1 - v / max);
  const peak = buckets.findIndex((b) => b.count === max);
  const p = stats.pause;
  return (
    <div className="gc-hist" data-testid="gc-histogram">
      {p && (
        <p className="gc-hist-caption">
          {t("gc.hist.percentiles", { p50: formatMs(p.p50), p95: formatMs(p.p95), p99: formatMs(p.p99), max: formatMs(p.max) })}
        </p>
      )}
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={t("gc.chart.histogram")} className="gc-hist-svg">
        <line x1={pad.l} x2={W - pad.r} y1={H - pad.b} y2={H - pad.b} stroke={CHROME.axis} strokeWidth={1} />
        {buckets.map((b, i) => {
          const cx = pad.l + slot * (i + 0.5);
          const top = y(b.count);
          const h = H - pad.b - top;
          const r = Math.min(4, h);
          return (
            <g
              key={b.lo_ms}
              onPointerEnter={() => setHover(i)}
              onPointerLeave={() => setHover(null)}
              data-testid={`gc-hist-bar-${i}`}
            >
              {/* Hit target is the whole column slot, not only the painted bar. */}
              <rect x={cx - slot / 2} y={pad.t} width={slot} height={H - pad.t - pad.b} fill="transparent" />
              {b.count > 0 && (
                <path
                  d={`M${cx - bw / 2},${H - pad.b} V${top + r} Q${cx - bw / 2},${top} ${cx - bw / 2 + r},${top} H${cx + bw / 2 - r} Q${cx + bw / 2},${top} ${cx + bw / 2},${top + r} V${H - pad.b} Z`}
                  fill={BAR_COLOR}
                  opacity={hover == null || hover === i ? 1 : 0.55}
                />
              )}
              {(i === peak || hover === i) && b.count > 0 && (
                <text x={cx} y={top - 6} textAnchor="middle" className="gc-hist-value">
                  {b.count}
                </text>
              )}
              <text x={cx} y={H - pad.b + 14} textAnchor="end" transform={`rotate(-35 ${cx} ${H - pad.b + 14})`} className="gc-hist-tick">
                {bucketLabel(b.lo_ms, b.hi_ms)}
              </text>
            </g>
          );
        })}
      </svg>
      {hover != null && buckets[hover] && (
        <p className="gc-hist-hover" role="status">
          {t("gc.hist.bucket", {
            range: bucketLabel(buckets[hover].lo_ms, buckets[hover].hi_ms),
            count: buckets[hover].count,
            pct: formatPct((100 * buckets[hover].count) / total),
          })}
        </p>
      )}
    </div>
  );
}

export function CauseBars({ causes }: { causes: CauseStat[] }) {
  const { t } = useI18n();
  if (causes.length === 0) return <p className="empty">{t("gc.noEvents")}</p>;
  const shown = causes.slice(0, MAX_CAUSES);
  const rest = causes.slice(MAX_CAUSES);
  const rows = rest.length
    ? [
        ...shown,
        {
          cause: t("gc.causes.other", { count: rest.length }),
          count: rest.reduce((a, c) => a + c.count, 0),
          total_pause_ms: rest.reduce((a, c) => a + c.total_pause_ms, 0),
          max_pause_ms: Math.max(...rest.map((c) => c.max_pause_ms)),
        },
      ]
    : shown;
  const max = Math.max(...rows.map((r) => r.total_pause_ms), 1e-9);
  return (
    <ul className="gc-causes" data-testid="gc-causes">
      {rows.map((c) => (
        <li key={c.cause} className="gc-cause" title={t("gc.causes.tip", { max: formatMs(c.max_pause_ms) })}>
          <span className="gc-cause-name">{c.cause}</span>
          <span className="gc-cause-track">
            <span className="gc-cause-bar" style={{ width: `${Math.max((70 * c.total_pause_ms) / max, 0.4)}%`, background: BAR_COLOR }} />
            <span className="gc-cause-value">
              {formatMs(c.total_pause_ms)} · {c.count}×
            </span>
          </span>
        </li>
      ))}
    </ul>
  );
}

interface TimeProps {
  markRow?: number | null;
  series: GcSeries;
  axis: TimeAxis;
  wallOffsetMs: number | null;
  xRange: [number, number] | null;
  onSelect: (x0: number, x1: number) => void;
  onReset: () => void;
  syncKey: string;
}

function xValues(axis: TimeAxis): uPlot.Axis["values"] {
  return axis === "wall" ? undefined : (_u, splits) => splits.map((s) => formatMs(s * 1000));
}

function timeLabel(x: number, axis: TimeAxis): string {
  return axis === "wall" ? formatWall(x * 1000) : formatMs(x * 1000);
}

export function RateChart({ series, axis, wallOffsetMs, xRange, onSelect, onReset, syncKey, markRow }: TimeProps) {
  const { t } = useI18n();
  const rate = useMemo(() => buildRateData(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const options = useCallback(
    (width: number): uPlot.Options => ({
      width,
      height: 0,
      scales: { x: { time: axis === "wall" }, y: { range: (_u, _min, max) => [0, max * 1.08 || 1] } },
      series: [
        {},
        { stroke: RATE_COLORS.allocation, width: 2, spanGaps: true, points: { show: false } },
        { stroke: RATE_COLORS.promotion, width: 2, spanGaps: true, points: { show: false } },
      ],
      axes: [baseAxis(xValues(axis)), { ...baseAxis((_u, splits) => splits.map((s) => formatRate(s))), size: 76 }],
      cursor: { sync: { key: syncKey, scales: ["x", null] }, points: { size: 8, width: 2, stroke: CHROME.surface } },
    }),
    [axis, syncKey],
  );
  const tooltip = useCallback(
    (idx: number) => {
      const rows: TipRow[] = [];
      const a = rate.data[1][idx];
      const p = rate.data[2][idx];
      if (a != null) rows.push({ value: formatRate(a), label: t("gc.series.allocation"), color: RATE_COLORS.allocation });
      if (p != null) rows.push({ value: formatRate(p), label: t("gc.series.promotion"), color: RATE_COLORS.promotion });
      return rows.length ? { title: timeLabel(rate.data[0][idx], axis), rows } : null;
    },
    [rate, axis, t],
  );
  if (rate.rows.length === 0) return <p className="empty">{t("gc.rates.none")}</p>;
  return (
    <>
      <Legend
        items={[
          { color: RATE_COLORS.allocation, label: t("gc.series.allocation") },
          ...(rate.hasPromotion ? [{ color: RATE_COLORS.promotion, label: t("gc.series.promotion") }] : []),
        ]}
      />
      <GcChart
        options={options}
        data={rate.data as uPlot.AlignedData}
        xRange={xRange}
        onSelect={onSelect}
        onReset={onReset}
        tooltip={tooltip}
        height={220}
        testId="gc-rate-chart"
        markX={markRow != null ? toX(series.time_ms[markRow], axis, wallOffsetMs) : null}
      />
    </>
  );
}

export function MetaspaceChart({ series, axis, wallOffsetMs, xRange, onSelect, onReset, syncKey, markRow }: TimeProps) {
  const { t } = useI18n();
  const meta = useMemo(() => buildMetaspaceData(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const options = useCallback(
    (width: number): uPlot.Options => ({
      width,
      height: 0,
      scales: { x: { time: axis === "wall" }, y: { range: (_u, _min, max) => [0, max * 1.08 || 1] } },
      series: [
        {},
        { stroke: HEAP_COLORS.used, width: 2, points: { show: false } },
        { stroke: CHROME.muted, width: 1, spanGaps: true, points: { show: false } },
      ],
      axes: [baseAxis(xValues(axis)), { ...baseAxis((_u, splits) => splits.map((s) => formatKb(s * 1024))), size: 72 }],
      cursor: { sync: { key: syncKey, scales: ["x", null] }, points: { size: 8, width: 2, stroke: CHROME.surface } },
    }),
    [axis, syncKey],
  );
  const tooltip = useCallback(
    (idx: number) => {
      const rows: TipRow[] = [{ value: formatKb(meta.data[1][idx] * 1024), label: t("gc.series.metaspaceUsed"), color: HEAP_COLORS.used }];
      const c = meta.data[2][idx];
      if (c != null) rows.push({ value: formatKb(c * 1024), label: t("gc.series.metaspaceCommitted"), color: CHROME.muted });
      return { title: timeLabel(meta.data[0][idx], axis), rows };
    },
    [meta, axis, t],
  );
  if (meta.rows.length === 0) return <p className="empty">{t("gc.metaspace.none")}</p>;
  return (
    <>
      <Legend
        items={[
          { color: HEAP_COLORS.used, label: t("gc.series.metaspaceUsed") },
          ...(meta.hasCommitted ? [{ color: CHROME.muted, label: t("gc.series.metaspaceCommitted") }] : []),
        ]}
      />
      <GcChart
        options={options}
        data={meta.data as uPlot.AlignedData}
        xRange={xRange}
        onSelect={onSelect}
        onReset={onReset}
        tooltip={tooltip}
        height={200}
        testId="gc-metaspace-chart"
        markX={markRow != null ? toX(series.time_ms[markRow], axis, wallOffsetMs) : null}
      />
    </>
  );
}
