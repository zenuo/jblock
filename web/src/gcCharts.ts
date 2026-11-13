/**
 * Chart data builders for the GC views (feat-075). Pure functions over the
 * columnar series from the worker; Node-testable.
 *
 * X values are seconds: uptime seconds on the uptime axis, epoch seconds on the
 * wall-clock axis (what uPlot's time scale expects).
 */

import type uPlot from "uplot";
import { GC_KINDS, type GcKind, type GcSeries } from "./gcTypes.ts";

export type TimeAxis = "uptime" | "wall";

/** Validated categorical slots (dataviz reference palette, light, all-pairs for 3). */
export const SERIES_COLORS = {
  young: "#2a78d6",
  full: "#eb6834",
  mixed: "#1baf7a",
} as const;

/**
 * Heap chart lines (adjacent-validated with the Full GC orange: violet, green).
 * They avoid blue / aqua so a color never means two entities on one page;
 * Full GC keeps its orange as full-height hairlines (a distinct mark form).
 */
export const HEAP_COLORS = {
  used: "#4a3aa7",
  old: "#008300",
} as const;

/**
 * Colors follow the concept across charts: violet = memory used (heap, metaspace)
 * and the allocation that fills it; green = old generation and the promotion
 * that fills it; muted gray = capacity / committed. Magnitude-only bars
 * (histogram, causes) use secondary ink so they never borrow an entity color.
 */
export const RATE_COLORS = {
  allocation: HEAP_COLORS.used,
  promotion: HEAP_COLORS.old,
} as const;

/** Chart chrome from the reference palette (light). */
export const CHROME = {
  surface: "#fcfcfb",
  ink: "#0b0b0b",
  secondary: "#52514e",
  muted: "#898781",
  grid: "#e1e0d9",
  axis: "#c3c2b7",
  /** Neutral wash for concurrent-cycle bands. */
  band: "rgba(137, 135, 129, 0.12)",
} as const;

/** Display bucket of a pause kind on the timeline (3 categorical + neutral). */
export type PauseBucket = "young" | "mixed" | "full" | "other";

export function pauseBucket(kind: GcKind): PauseBucket {
  if (kind === "young") return "young";
  if (kind === "mixed") return "mixed";
  if (kind === "full") return "full";
  return "other";
}

export function bucketColor(bucket: PauseBucket): string {
  return bucket === "other" ? CHROME.muted : SERIES_COLORS[bucket];
}

export function kindAt(series: GcSeries, i: number): GcKind {
  return GC_KINDS[series.kind[i]] ?? "other-pause";
}

/** Convert a time-axis value (ms) to a chart x (s). */
export function toX(timeMs: number, axis: TimeAxis, wallOffsetMs: number | null): number {
  return axis === "wall" && wallOffsetMs != null ? (timeMs + wallOffsetMs) / 1000 : timeMs / 1000;
}

/** Convert a chart x (s) back to the time axis (ms). */
export function fromX(x: number, axis: TimeAxis, wallOffsetMs: number | null): number {
  return axis === "wall" && wallOffsetMs != null ? x * 1000 - wallOffsetMs : x * 1000;
}

export interface PauseData {
  /** uPlot aligned data: [x, duration_ms]. */
  data: [number[], number[]];
  /** Series row of each point. */
  rows: number[];
  /** Pause bucket of each point. */
  buckets: PauseBucket[];
}

/** One point per pause event, time-ordered. */
export function buildPauseData(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): PauseData {
  const xs: number[] = [];
  const ys: number[] = [];
  const rows: number[] = [];
  const buckets: PauseBucket[] = [];
  const n = series.time_ms.length;
  for (let i = 0; i < n; i++) {
    if (series.pause[i] !== 1) continue;
    xs.push(toX(series.time_ms[i], axis, wallOffsetMs));
    ys.push(series.duration_ms[i]);
    rows.push(i);
    buckets.push(pauseBucket(kindAt(series, i)));
  }
  return { data: [xs, ys], rows, buckets };
}

export interface HeapData {
  /**
   * uPlot aligned data: [x, heap used, old gen after GC, heap capacity].
   * Each pause with heap info contributes two points: before at its start and
   * after at its end, so the used line draws the GC sawtooth.
   */
  data: [number[], (number | null)[], (number | null)[], (number | null)[]];
  /** Series row of each point. */
  rows: number[];
  hasOld: boolean;
  hasCapacity: boolean;
}

const MB = 1024;

export function buildHeapData(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): HeapData {
  const xs: number[] = [];
  const used: (number | null)[] = [];
  const old: (number | null)[] = [];
  const cap: (number | null)[] = [];
  const rows: number[] = [];
  let hasOld = false;
  let hasCapacity = false;
  const n = series.time_ms.length;
  for (let i = 0; i < n; i++) {
    const before = series.heap_before_kb[i];
    const after = series.heap_after_kb[i];
    // Pauses and, for ZGC / Shenandoah, the cycles that carry the heap figures.
    if (!Number.isFinite(before) || !Number.isFinite(after)) continue;
    const start = series.time_ms[i];
    const capacity = series.heap_capacity_kb[i];
    const oldAfter = series.old_after_kb[i];
    const c = Number.isFinite(capacity) ? capacity / MB : null;
    hasCapacity ||= c != null;
    hasOld ||= Number.isFinite(oldAfter);
    xs.push(toX(start, axis, wallOffsetMs));
    used.push(before / MB);
    old.push(null);
    cap.push(c);
    rows.push(i);
    xs.push(toX(start + series.duration_ms[i], axis, wallOffsetMs));
    used.push(after / MB);
    old.push(Number.isFinite(oldAfter) ? oldAfter / MB : null);
    cap.push(c);
    rows.push(i);
  }
  return { data: [xs, used, old, cap], rows, hasOld, hasCapacity };
}

/** Concurrent cycles as [x0, x1] bands. */
export function concurrentBands(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): [number, number][] {
  const out: [number, number][] = [];
  const concurrent = GC_KINDS.indexOf("concurrent");
  for (let i = 0; i < series.time_ms.length; i++) {
    if (series.kind[i] !== concurrent) continue;
    const t = series.time_ms[i];
    out.push([toX(t, axis, wallOffsetMs), toX(t + series.duration_ms[i], axis, wallOffsetMs)]);
  }
  return out;
}

/** X positions of Full GC pauses (heap chart markers). */
export function fullGcXs(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): number[] {
  const full = GC_KINDS.indexOf("full");
  const out: number[] = [];
  for (let i = 0; i < series.time_ms.length; i++) {
    if (series.kind[i] === full && series.pause[i] === 1) out.push(toX(series.time_ms[i], axis, wallOffsetMs));
  }
  return out;
}

/** First index with xs[i] >= x (xs ascending). */
export function lowerBound(xs: ArrayLike<number>, x: number): number {
  let lo = 0;
  let hi = xs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (xs[mid] < x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Use a log y-scale when pauses span more than two orders of magnitude. */
export function preferLogScale(durations: ArrayLike<number>): boolean {
  let min = Infinity;
  let max = 0;
  for (let i = 0; i < durations.length; i++) {
    const d = durations[i];
    if (d > 0 && d < min) min = d;
    if (d > max) max = d;
  }
  return Number.isFinite(min) && max / min > 100;
}

/** Shared axis chrome (dataviz reference: hairline grid, muted ticks). */
export function baseAxis(values?: uPlot.Axis["values"]): uPlot.Axis {
  return {
    stroke: CHROME.muted,
    grid: { stroke: CHROME.grid, width: 1 },
    ticks: { stroke: CHROME.axis, width: 1, size: 4 },
    font: '11px system-ui, -apple-system, "Segoe UI", sans-serif',
    values,
  };
}

export interface RateData {
  /** uPlot aligned data: [x, allocation MB/s, promotion MB/s]. */
  data: [number[], (number | null)[], (number | null)[]];
  rows: number[];
  hasPromotion: boolean;
}

/** Allocation / promotion rate at each pause that has one (same formulas as the Rust stats). */
export function buildRateData(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): RateData {
  const xs: number[] = [];
  const alloc: (number | null)[] = [];
  const promo: (number | null)[] = [];
  const rows: number[] = [];
  let hasPromotion = false;
  const a = series.alloc_rate_mb_s;
  const p = series.promotion_rate_mb_s;
  for (let i = 0; i < series.time_ms.length; i++) {
    const av = Number.isFinite(a[i]) ? a[i] : null;
    const pv = p && Number.isFinite(p[i]) ? p[i] : null;
    if (av == null && pv == null) continue;
    hasPromotion ||= pv != null;
    xs.push(toX(series.time_ms[i], axis, wallOffsetMs));
    alloc.push(av);
    promo.push(pv);
    rows.push(i);
  }
  return { data: [xs, alloc, promo], rows, hasPromotion };
}

export interface MetaspaceData {
  /** uPlot aligned data: [x, used MB, committed MB]. */
  data: [number[], number[], (number | null)[]];
  rows: number[];
  hasCommitted: boolean;
}

export function buildMetaspaceData(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null): MetaspaceData {
  const xs: number[] = [];
  const used: number[] = [];
  const committed: (number | null)[] = [];
  const rows: number[] = [];
  let hasCommitted = false;
  for (let i = 0; i < series.time_ms.length; i++) {
    const u = series.metaspace_after_kb[i];
    if (!Number.isFinite(u)) continue;
    const c = series.metaspace_capacity_kb[i];
    hasCommitted ||= Number.isFinite(c);
    xs.push(toX(series.time_ms[i] + series.duration_ms[i], axis, wallOffsetMs));
    used.push(u / MB);
    committed.push(Number.isFinite(c) ? c / MB : null);
    rows.push(i);
  }
  return { data: [xs, used, committed], rows, hasCommitted };
}

/** Bucket label: `0.1–0.2 ms`, `1–2 s`, `≥ 5 s`. */
export function bucketLabel(lo: number, hi: number | null): string {
  const fmt = (ms: number) => (ms >= 1000 ? `${ms / 1000}` : `${ms}`);
  const unit = (ms: number) => (ms >= 1000 ? "s" : "ms");
  if (hi == null) return `≥ ${fmt(lo)} ${unit(lo)}`;
  if (unit(lo) === unit(hi)) return `${fmt(lo)}–${fmt(hi)} ${unit(hi)}`;
  return `${fmt(lo)} ${unit(lo)}–${fmt(hi)} ${unit(hi)}`;
}

/** Comparison table: index of the worst value (null when values are missing or all equal). */
export function worstRow(values: (number | null)[], worse: "high" | "low"): number | null {
  let best: number | null = null;
  values.forEach((v, i) => {
    if (v == null || !Number.isFinite(v)) return;
    if (best == null) best = i;
    else if (worse === "high" ? v > (values[best] as number) : v < (values[best] as number)) best = i;
  });
  const finite = values.filter((v) => v != null && Number.isFinite(v));
  return finite.length >= 2 && new Set(finite).size > 1 ? best : null;
}
