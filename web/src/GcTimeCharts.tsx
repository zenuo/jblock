import { useCallback, useMemo } from "react";
import type uPlot from "uplot";
import GcChart, { type TipRow } from "./GcChart";
import {
  CHROME,
  HEAP_COLORS,
  baseAxis,
  SERIES_COLORS,
  bucketColor,
  buildHeapData,
  buildPauseData,
  concurrentBands,
  fullGcXs,
  kindAt,
  lowerBound,
  toX,
  type PauseBucket,
  type TimeAxis,
} from "./gcCharts";
import { formatKb, formatMs, formatWall } from "./gcFormat";
import { FLAG_EVACUATION_FAILURE, type GcSeries } from "./gcTypes";
import { useI18n, type MessageKey } from "./i18n";

interface Common {
  /** Series row of the selected event (table selection). */
  markRow?: number | null;
  /** Clicked a point: the series row of that event. */
  onPickRow?: (row: number) => void;
  series: GcSeries;
  axis: TimeAxis;
  wallOffsetMs: number | null;
  xRange: [number, number] | null;
  onSelect: (x0: number, x1: number) => void;
  onReset: () => void;
  syncKey: string;
}

/** Draw order: neutral first, Full GC last so it is never hidden. */
const DRAW_ORDER: PauseBucket[] = ["other", "young", "mixed", "full"];
const LEGEND: { bucket: PauseBucket; key: MessageKey }[] = [
  { bucket: "young", key: "gc.series.young" },
  { bucket: "mixed", key: "gc.series.mixed" },
  { bucket: "full", key: "gc.series.full" },
  { bucket: "other", key: "gc.series.other" },
];

function xValues(axis: TimeAxis): uPlot.Axis["values"] {
  return axis === "wall" ? undefined : (_u, splits) => splits.map((s) => formatMs(s * 1000));
}

function timeLabel(x: number, axis: TimeAxis): string {
  return axis === "wall" ? formatWall(x * 1000) : formatMs(x * 1000);
}

function decades(min: number, max: number): number[] {
  const out: number[] = [];
  for (let e = Math.floor(Math.log10(Math.max(min, 1e-6))); e <= Math.ceil(Math.log10(Math.max(max, 1e-6))); e++) {
    out.push(10 ** e);
  }
  return out;
}

function drawBands(bands: [number, number][]) {
  return (u: uPlot) => {
    if (bands.length === 0) return;
    const { ctx, bbox } = u;
    const min = u.scales.x.min ?? -Infinity;
    const max = u.scales.x.max ?? Infinity;
    ctx.save();
    ctx.fillStyle = CHROME.band;
    for (const [a, b] of bands) {
      if (b < min || a > max) continue;
      const x0 = Math.max(u.valToPos(a, "x", true), bbox.left);
      const x1 = Math.min(u.valToPos(b, "x", true), bbox.left + bbox.width);
      // Sub-pixel cycles would only read as stray gridlines; they stay in the event table.
      if (x1 - x0 < devicePixelRatio) continue;
      ctx.fillRect(x0, bbox.top, x1 - x0, bbox.height);
    }
    ctx.restore();
  };
}

function Legend({ items }: { items: { color: string; label: string; shape: "dot" | "line" | "band" | "tri" }[] }) {
  return (
    <ul className="gc-legend">
      {items.map((it) => (
        <li key={it.label}>
          <span className={`gc-legend-key ${it.shape}`} style={{ background: it.shape === "band" ? CHROME.band : it.color }} />
          {it.label}
        </li>
      ))}
    </ul>
  );
}

export function PauseTimeline({ series, axis, wallOffsetMs, xRange, onSelect, onReset, syncKey, logScale, markRow, onPickRow }: Common & { logScale: boolean }) {
  const { t } = useI18n();
  const pause = useMemo(() => buildPauseData(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const bands = useMemo(() => concurrentBands(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const data = useMemo(
    () => [pause.data[0], logScale ? pause.data[1].map((v) => Math.max(v, 0.001)) : pause.data[1]] as uPlot.AlignedData,
    [pause, logScale],
  );

  const options = useCallback(
    (width: number): uPlot.Options => {
      const xs = pause.data[0];
      const drawPoints = (u: uPlot) => {
        const { ctx, bbox } = u;
        const dpr = devicePixelRatio;
        const i0 = lowerBound(xs, u.scales.x.min ?? -Infinity);
        const i1 = lowerBound(xs, (u.scales.x.max ?? Infinity) + 1e-9);
        const ys = u.data[1] as number[];
        ctx.save();
        ctx.beginPath();
        ctx.rect(bbox.left, bbox.top, bbox.width, bbox.height);
        ctx.clip();
        ctx.lineWidth = 2 * dpr;
        ctx.strokeStyle = CHROME.surface;
        for (const bucket of DRAW_ORDER) {
          ctx.fillStyle = bucketColor(bucket);
          const r = (bucket === "full" ? 6 : 4) * dpr;
          for (let i = i0; i < i1; i++) {
            if (pause.buckets[i] !== bucket) continue;
            const cx = u.valToPos(xs[i], "x", true);
            const cy = u.valToPos(ys[i], "y", true);
            ctx.beginPath();
            if (bucket === "full") {
              // Secondary encoding: Full GC is a larger triangle, not only a color.
              ctx.moveTo(cx, cy - r);
              ctx.lineTo(cx + r, cy + r * 0.8);
              ctx.lineTo(cx - r, cy + r * 0.8);
              ctx.closePath();
            } else {
              ctx.arc(cx, cy, r, 0, Math.PI * 2);
            }
            ctx.stroke();
            ctx.fill();
          }
        }
        ctx.restore();
      };
      return {
        width,
        height: 0,
        scales: {
          x: { time: axis === "wall" },
          y: logScale ? { distr: 3, log: 10 } : { range: (_u, _min, max) => [0, max * 1.08 || 1] },
        },
        series: [{}, { stroke: "transparent", paths: () => null, points: { show: false } }],
        axes: [
          baseAxis(xValues(axis)),
          {
            ...baseAxis((_u, splits) => splits.map((s) => formatMs(s))),
            size: 64,
            // Log scale: ticks and gridlines only at powers of ten (no 2…9 minor clutter).
            ...(logScale ? { splits: (_u: uPlot, _i: number, min: number, max: number) => decades(min, max) } : {}),
          },
        ],
        cursor: {
          sync: { key: syncKey, scales: ["x", null] },
          points: {
            size: 12,
            width: 2,
            stroke: CHROME.surface,
            fill: (u: uPlot) => {
              const i = u.cursor.idx;
              return i == null ? CHROME.muted : bucketColor(pause.buckets[i]);
            },
          },
        },
        hooks: { drawClear: [drawBands(bands)], draw: [drawPoints] },
      };
    },
    [pause, bands, axis, syncKey, logScale],
  );

  const tooltip = useCallback(
    (idx: number) => {
      const row = pause.rows[idx];
      if (row == null) return null;
      const kind = kindAt(series, row);
      const id = series.gc_id[row];
      const rows: TipRow[] = [
        { value: formatMs(series.duration_ms[row]), label: t(`gc.kind.${kind}` as MessageKey), color: bucketColor(pause.buckets[idx]) },
      ];
      if (Number.isFinite(series.heap_before_kb[row])) {
        rows.push({
          value: t("gc.tip.heap", { before: formatKb(series.heap_before_kb[row]), after: formatKb(series.heap_after_kb[row]) }),
          label: "",
        });
      }
      if ((series.flags[row] & FLAG_EVACUATION_FAILURE) !== 0) rows.push({ value: t("gc.tip.evacFailure"), label: "" });
      const title = `${timeLabel(pause.data[0][idx], axis)}${Number.isFinite(id) ? ` · ${t("gc.tip.gcId", { id })}` : ""}`;
      return { title, rows };
    },
    [pause, series, axis, t],
  );

  const legend = [
    ...LEGEND.filter((l) => pause.buckets.includes(l.bucket)).map((l) => ({
      color: bucketColor(l.bucket),
      label: t(l.key),
      shape: (l.bucket === "full" ? "tri" : "dot") as "tri" | "dot",
    })),
    ...(bands.length > 0 ? [{ color: CHROME.band, label: t("gc.series.concurrent"), shape: "band" as const }] : []),
  ];

  return (
    <>
      <Legend items={legend} />
      <GcChart
        options={options}
        data={data}
        xRange={xRange}
        onSelect={onSelect}
        onReset={onReset}
        tooltip={tooltip}
        height={260}
        testId="gc-pause-chart"
        markX={markRow != null ? toX(series.time_ms[markRow], axis, wallOffsetMs) : null}
        onPick={onPickRow ? (idx) => onPickRow(pause.rows[idx]) : undefined}
      />
    </>
  );
}

export function HeapChart({ series, axis, wallOffsetMs, xRange, onSelect, onReset, syncKey, markRow, onPickRow }: Common) {
  const { t } = useI18n();
  const heap = useMemo(() => buildHeapData(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const bands = useMemo(() => concurrentBands(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const fulls = useMemo(() => fullGcXs(series, axis, wallOffsetMs), [series, axis, wallOffsetMs]);
  const data = heap.data as uPlot.AlignedData;

  const options = useCallback(
    (width: number): uPlot.Options => {
      const drawFull = (u: uPlot) => {
        if (fulls.length === 0) return;
        const { ctx, bbox } = u;
        ctx.save();
        ctx.strokeStyle = SERIES_COLORS.full;
        ctx.lineWidth = 1 * devicePixelRatio;
        for (const x of fulls) {
          const px = Math.round(u.valToPos(x, "x", true)) + 0.5;
          if (px < bbox.left || px > bbox.left + bbox.width) continue;
          ctx.beginPath();
          ctx.moveTo(px, bbox.top);
          ctx.lineTo(px, bbox.top + bbox.height);
          ctx.stroke();
        }
        ctx.restore();
      };
      return {
        width,
        height: 0,
        scales: { x: { time: axis === "wall" }, y: { range: (_u, _min, max) => [0, max * 1.08 || 1] } },
        series: [
          {},
          { stroke: HEAP_COLORS.used, width: 2, points: { show: false } },
          { stroke: HEAP_COLORS.old, width: 2, spanGaps: true, points: { show: false } },
          { stroke: CHROME.muted, width: 1, points: { show: false } },
        ],
        axes: [baseAxis(xValues(axis)), { ...baseAxis((_u, splits) => splits.map((s) => formatKb(s * 1024))), size: 72 }],
        cursor: { sync: { key: syncKey, scales: ["x", null] }, points: { size: 8, width: 2, stroke: CHROME.surface } },
        hooks: { drawClear: [drawBands(bands)], draw: [drawFull] },
      };
    },
    [bands, fulls, axis, syncKey],
  );

  const tooltip = useCallback(
    (idx: number) => {
      const row = heap.rows[idx];
      if (row == null) return null;
      const kind = kindAt(series, row);
      const id = series.gc_id[row];
      const rows: TipRow[] = [];
      const used = heap.data[1][idx];
      if (used != null) rows.push({ value: formatKb(used * 1024), label: t("gc.series.heapUsed"), color: HEAP_COLORS.used });
      const old = heap.data[2][idx];
      if (old != null) rows.push({ value: formatKb(old * 1024), label: t("gc.series.oldAfter"), color: HEAP_COLORS.old });
      const cap = heap.data[3][idx];
      if (cap != null) rows.push({ value: formatKb(cap * 1024), label: t("gc.series.capacity"), color: CHROME.muted });
      const title = `${timeLabel(heap.data[0][idx], axis)} · ${t(`gc.kind.${kind}` as MessageKey)}${Number.isFinite(id) ? ` · ${t("gc.tip.gcId", { id })}` : ""}`;
      return { title, rows };
    },
    [heap, series, axis, t],
  );

  const legend = [
    { color: HEAP_COLORS.used, label: t("gc.series.heapUsed"), shape: "line" as const },
    ...(heap.hasOld ? [{ color: HEAP_COLORS.old, label: t("gc.series.oldAfter"), shape: "line" as const }] : []),
    ...(heap.hasCapacity ? [{ color: CHROME.muted, label: t("gc.series.capacity"), shape: "line" as const }] : []),
    ...(fulls.length > 0 ? [{ color: SERIES_COLORS.full, label: t("gc.series.full"), shape: "line" as const }] : []),
    ...(bands.length > 0 ? [{ color: CHROME.band, label: t("gc.series.concurrent"), shape: "band" as const }] : []),
  ];

  return (
    <>
      <Legend items={legend} />
      <GcChart
        options={options}
        data={data}
        xRange={xRange}
        onSelect={onSelect}
        onReset={onReset}
        tooltip={tooltip}
        height={260}
        testId="gc-heap-chart"
        markX={markRow != null ? toX(series.time_ms[markRow], axis, wallOffsetMs) : null}
        onPick={onPickRow ? (idx) => onPickRow(heap.rows[idx]) : undefined}
      />
    </>
  );
}
