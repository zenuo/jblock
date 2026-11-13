/**
 * Static SVG charts for the GC HTML export (feat-079). Pure string builders;
 * Node-testable. Series are downsampled to at most `MAX_POINTS` per chart
 * (min/max per pixel column for lines, longest pause per column for points).
 */

import {
  CHROME,
  HEAP_COLORS,
  SERIES_COLORS,
  bucketColor,
  bucketLabel,
  buildHeapData,
  buildMetaspaceData,
  buildPauseData,
  buildRateData,
  concurrentBands,
  fullGcXs,
  type TimeAxis,
} from "./gcCharts.ts";
import { formatKb, formatMs, formatRate, formatWall } from "./gcFormat.ts";
import type { GcSeries, HistogramBucket } from "./gcTypes.ts";

export const MAX_POINTS = 2000;
const W = 1000;
const H = 240;
const PAD = { l: 64, r: 12, t: 10, b: 26 };

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

const r1 = (v: number) => Math.round(v * 10) / 10;

/** Min / max of finite values without spreading (huge arrays overflow the call stack). */
export function extent(values: Iterable<number | null>): [number, number] {
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of values) {
    if (v == null || !Number.isFinite(v)) continue;
    if (v < lo) lo = v;
    if (v > hi) hi = v;
  }
  return [lo, hi];
}

/** Min/max per bucket of x so a line keeps its peaks and troughs. */
export function downsampleMinMax(xs: number[], ys: (number | null)[], buckets: number): [number, number][] {
  const pts: [number, number][] = [];
  for (let i = 0; i < xs.length; i++) {
    const y = ys[i];
    if (y != null && Number.isFinite(y)) pts.push([xs[i], y]);
  }
  if (pts.length <= buckets * 2) return pts;
  const x0 = pts[0][0];
  const span = pts[pts.length - 1][0] - x0 || 1;
  const out: [number, number][] = [];
  let cur = -1;
  let min: [number, number] | null = null;
  let max: [number, number] | null = null;
  const flush = () => {
    if (min && max) {
      if (min[0] <= max[0]) out.push(min, max);
      else out.push(max, min);
      if (min === max) out.pop();
    }
  };
  for (const p of pts) {
    const b = Math.min(buckets - 1, Math.floor(((p[0] - x0) / span) * buckets));
    if (b !== cur) {
      flush();
      cur = b;
      min = max = p;
    } else {
      if (p[1] < min![1]) min = p;
      if (p[1] > max![1]) max = p;
    }
  }
  flush();
  return out;
}

/** Keep the largest y per bucket (plus every point flagged `keep`). */
export function downsampleMax<T extends { x: number; y: number; keep?: boolean }>(pts: T[], buckets: number): T[] {
  if (pts.length <= buckets) return pts;
  const [x0, xMax] = extent(pts.map((p) => p.x));
  const span = xMax - x0 || 1;
  const best = new Map<number, T>();
  const kept: T[] = [];
  for (const p of pts) {
    if (p.keep) {
      kept.push(p);
      continue;
    }
    const b = Math.min(buckets - 1, Math.floor(((p.x - x0) / span) * buckets));
    const cur = best.get(b);
    if (!cur || p.y > cur.y) best.set(b, p);
  }
  return [...best.values(), ...kept].sort((a, b) => a.x - b.x);
}

function niceTicks(lo: number, hi: number, n: number): number[] {
  if (!(hi > lo)) return [lo];
  const raw = (hi - lo) / n;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) out.push(v);
  return out;
}

interface Frame {
  x0: number;
  x1: number;
  y0: number;
  y1: number;
  log: boolean;
}

function sx(f: Frame, x: number): number {
  return r1(PAD.l + ((x - f.x0) / (f.x1 - f.x0 || 1)) * (W - PAD.l - PAD.r));
}

function sy(f: Frame, y: number): number {
  const t = f.log
    ? (Math.log10(Math.max(y, f.y0)) - Math.log10(f.y0)) / (Math.log10(f.y1) - Math.log10(f.y0) || 1)
    : (y - f.y0) / (f.y1 - f.y0 || 1);
  return r1(H - PAD.b - t * (H - PAD.t - PAD.b));
}

function axes(f: Frame, xLabel: (x: number) => string, yLabel: (y: number) => string): string {
  const yt = f.log
    ? Array.from({ length: Math.floor(Math.log10(f.y1)) - Math.ceil(Math.log10(f.y0)) + 1 }, (_, i) => 10 ** (Math.ceil(Math.log10(f.y0)) + i))
    : niceTicks(f.y0, f.y1, 4);
  const xt = niceTicks(f.x0, f.x1, 6);
  const grid = yt
    .map((y) => `<line x1="${PAD.l}" x2="${W - PAD.r}" y1="${sy(f, y)}" y2="${sy(f, y)}" stroke="${CHROME.grid}" stroke-width="1"/><text x="${PAD.l - 6}" y="${sy(f, y) + 3}" text-anchor="end">${esc(yLabel(y))}</text>`)
    .join("");
  const xs = xt.map((x) => `<text x="${sx(f, x)}" y="${H - 8}" text-anchor="middle">${esc(xLabel(x))}</text>`).join("");
  return `<g class="gc-svg-axis" fill="${CHROME.muted}" font-size="10">${grid}${xs}<line x1="${PAD.l}" x2="${W - PAD.r}" y1="${H - PAD.b}" y2="${H - PAD.b}" stroke="${CHROME.axis}"/></g>`;
}

function svgOpen(label: string): string {
  return `<svg class="gc-svg" viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(label)}" xmlns="http://www.w3.org/2000/svg" font-family="system-ui, -apple-system, Segoe UI, sans-serif">`;
}

function xLabeler(axis: TimeAxis): (x: number) => string {
  return axis === "wall" ? (x) => formatWall(x * 1000).slice(11) : (x) => formatMs(x * 1000);
}

function bandsSvg(f: Frame, bands: [number, number][]): string {
  return bands
    .filter(([a, b]) => sx(f, b) - sx(f, a) >= 1)
    .map(([a, b]) => `<rect x="${sx(f, a)}" y="${PAD.t}" width="${r1(sx(f, b) - sx(f, a))}" height="${H - PAD.t - PAD.b}" fill="${CHROME.band}"/>`)
    .join("");
}

function polyline(f: Frame, pts: [number, number][], color: string, width: number): string {
  if (pts.length < 2) return "";
  return `<polyline fill="none" stroke="${color}" stroke-width="${width}" stroke-linejoin="round" points="${pts.map(([x, y]) => `${sx(f, x)},${sy(f, y)}`).join(" ")}"/>`;
}

export function pauseSvg(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null, label: string): string {
  const p = buildPauseData(series, axis, wallOffsetMs);
  if (p.rows.length === 0) return "";
  const pts = p.data[0].map((x, i) => ({ x, y: Math.max(p.data[1][i], 0.001), bucket: p.buckets[i], keep: p.buckets[i] === "full" }));
  const shown = downsampleMax(pts, MAX_POINTS);
  const [yLo, yHi] = extent(pts.map((q) => q.y));
  const log = yHi / yLo > 100;
  const f: Frame = {
    x0: p.data[0][0],
    x1: p.data[0][p.data[0].length - 1],
    y0: log ? 10 ** Math.floor(Math.log10(yLo)) : 0,
    y1: log ? 10 ** Math.ceil(Math.log10(yHi)) : yHi * 1.08,
    log,
  };
  const order = ["other", "young", "mixed", "full"];
  const marks = order
    .flatMap((b) => shown.filter((q) => q.bucket === b))
    .map((q) => {
      const cx = sx(f, q.x);
      const cy = sy(f, q.y);
      const c = bucketColor(q.bucket);
      return q.bucket === "full"
        ? `<path d="M${cx},${cy - 6} L${cx + 6},${r1(cy + 4.8)} L${cx - 6},${r1(cy + 4.8)} Z" fill="${c}" stroke="${CHROME.surface}" stroke-width="2"/>`
        : `<circle cx="${cx}" cy="${cy}" r="4" fill="${c}" stroke="${CHROME.surface}" stroke-width="2"/>`;
    })
    .join("");
  return `${svgOpen(label)}${bandsSvg(f, concurrentBands(series, axis, wallOffsetMs))}${axes(f, xLabeler(axis), formatMs)}${marks}</svg>`;
}

export function heapSvg(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null, label: string): string {
  const h = buildHeapData(series, axis, wallOffsetMs);
  if (h.rows.length === 0) return "";
  const xs = h.data[0];
  // Min/max keeps up to 2 points per bucket: 1/4 + 1/8 + 1/8 of the budget in buckets.
  const used = downsampleMinMax(xs, h.data[1], MAX_POINTS / 4);
  const old = downsampleMinMax(xs, h.data[2], MAX_POINTS / 8);
  const cap = downsampleMinMax(xs, h.data[3], MAX_POINTS / 8);
  const yMax = Math.max(extent(h.data[1])[1], extent(h.data[3])[1]) * 1.08;
  const f: Frame = { x0: xs[0], x1: xs[xs.length - 1], y0: 0, y1: yMax || 1, log: false };
  const fulls = fullGcXs(series, axis, wallOffsetMs)
    .map((x) => `<line x1="${sx(f, x)}" x2="${sx(f, x)}" y1="${PAD.t}" y2="${H - PAD.b}" stroke="${SERIES_COLORS.full}" stroke-width="1"/>`)
    .join("");
  return `${svgOpen(label)}${bandsSvg(f, concurrentBands(series, axis, wallOffsetMs))}${axes(f, xLabeler(axis), (y) => formatKb(y * 1024))}${fulls}${polyline(f, cap, CHROME.muted, 1)}${polyline(f, old, HEAP_COLORS.old, 2)}${polyline(f, used, HEAP_COLORS.used, 2)}</svg>`;
}

export function rateSvg(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null, label: string): string {
  const r = buildRateData(series, axis, wallOffsetMs);
  if (r.rows.length === 0) return "";
  const alloc = downsampleMinMax(r.data[0], r.data[1], MAX_POINTS / 4);
  const promo = downsampleMinMax(r.data[0], r.data[2], MAX_POINTS / 4);
  const yMax = Math.max(extent(alloc.map((p) => p[1]))[1], extent(promo.map((p) => p[1]))[1], 0) * 1.08 || 1;
  const f: Frame = { x0: r.data[0][0], x1: r.data[0][r.data[0].length - 1], y0: 0, y1: yMax, log: false };
  return `${svgOpen(label)}${axes(f, xLabeler(axis), formatRate)}${polyline(f, promo, HEAP_COLORS.old, 2)}${polyline(f, alloc, HEAP_COLORS.used, 2)}</svg>`;
}

export function metaspaceSvg(series: GcSeries, axis: TimeAxis, wallOffsetMs: number | null, label: string): string {
  const m = buildMetaspaceData(series, axis, wallOffsetMs);
  if (m.rows.length === 0) return "";
  const used = downsampleMinMax(m.data[0], m.data[1], MAX_POINTS / 4);
  const committed = downsampleMinMax(m.data[0], m.data[2], MAX_POINTS / 4);
  const yMax = Math.max(extent(used.map((p) => p[1]))[1], extent(committed.map((p) => p[1]))[1], 0) * 1.08 || 1;
  const f: Frame = { x0: m.data[0][0], x1: m.data[0][m.data[0].length - 1], y0: 0, y1: yMax, log: false };
  return `${svgOpen(label)}${axes(f, xLabeler(axis), (y) => formatKb(y * 1024))}${polyline(f, committed, CHROME.muted, 1)}${polyline(f, used, HEAP_COLORS.used, 2)}</svg>`;
}

export function histogramSvg(buckets: HistogramBucket[], label: string): string {
  if (buckets.length === 0) return "";
  const max = Math.max(...buckets.map((b) => b.count), 1);
  const w = 560;
  const h = 200;
  const pad = { l: 8, r: 8, t: 22, b: 40 };
  const slot = (w - pad.l - pad.r) / buckets.length;
  const bw = Math.min(24, slot * 0.7);
  const bars = buckets
    .map((b, i) => {
      const cx = r1(pad.l + slot * (i + 0.5));
      const top = r1(pad.t + (h - pad.t - pad.b) * (1 - b.count / max));
      const value = b.count === max ? `<text x="${cx}" y="${top - 6}" text-anchor="middle" font-size="11" font-weight="600" fill="${CHROME.ink}">${b.count}</text>` : "";
      const bar = b.count > 0 ? `<rect x="${r1(cx - bw / 2)}" y="${top}" width="${r1(bw)}" height="${r1(h - pad.b - top)}" rx="4" fill="${CHROME.secondary}"><title>${esc(`${bucketLabel(b.lo_ms, b.hi_ms)}: ${b.count}`)}</title></rect>` : "";
      return `${bar}${value}<text x="${cx}" y="${h - pad.b + 14}" text-anchor="end" transform="rotate(-35 ${cx} ${h - pad.b + 14})" font-size="10" fill="${CHROME.muted}">${esc(bucketLabel(b.lo_ms, b.hi_ms))}</text>`;
    })
    .join("");
  return `<svg class="gc-svg" viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(label)}" xmlns="http://www.w3.org/2000/svg" font-family="system-ui, -apple-system, Segoe UI, sans-serif"><line x1="${pad.l}" x2="${w - pad.r}" y1="${h - pad.b}" y2="${h - pad.b}" stroke="${CHROME.axis}"/>${bars}</svg>`;
}
