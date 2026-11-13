#!/usr/bin/env node
// Unit tests for web/src/gcSvg.ts (feat-079): downsampling bounds and SVG output.
import assert from "node:assert/strict";
import { MAX_POINTS, downsampleMax, downsampleMinMax, heapSvg, histogramSvg, pauseSvg } from "../web/src/gcSvg.ts";
import { GC_KINDS } from "../web/src/gcTypes.ts";

// 100k-point sawtooth keeps its extremes within the bucket budget.
const n = 100_000;
const xs = Array.from({ length: n }, (_, i) => i);
const ys = xs.map((i) => (i % 100) + (i === 54_321 ? 1000 : 0));
const mm = downsampleMinMax(xs, ys, 500);
assert.ok(mm.length <= 1000, `min/max points ${mm.length}`);
assert.equal(Math.max(...mm.map((p) => p[1])), 1021, "spike survives (54321 % 100 + 1000)");
assert.equal(Math.min(...mm.map((p) => p[1])), 0);
assert.deepEqual(downsampleMinMax([1, 2], [5, null], 10), [[1, 5]]);

const pts = xs.map((x) => ({ x, y: ys[x], keep: x % 25_000 === 0 }));
const mx = downsampleMax(pts, 300);
assert.ok(mx.length <= 300 + 4);
assert.ok(mx.some((p) => p.y === 1021));
assert.equal(mx.filter((p) => p.keep).length, 4, "kept points are never dropped");

// Synthetic series: 50k young pauses + Full GCs.
const K = (k) => GC_KINDS.indexOf(k);
const m = 50_000;
const f64 = (fn) => Float64Array.from({ length: m }, (_, i) => fn(i));
const series = {
  time_ms: f64((i) => i * 1000),
  duration_ms: f64((i) => (i % 1000 === 0 ? 900 : 2 + (i % 7))),
  kind: f64((i) => (i % 1000 === 0 ? K("full") : K("young"))),
  pause: f64(() => 1),
  heap_before_kb: f64((i) => 500_000 + (i % 10) * 1000),
  heap_after_kb: f64(() => 100_000),
  heap_capacity_kb: f64(() => 1_048_576),
  old_after_kb: f64((i) => 50_000 + i),
};
const ps = pauseSvg(series, "uptime", null, "Pauses");
assert.ok(ps.startsWith("<svg") && ps.endsWith("</svg>"));
const marks = (ps.match(/<circle /g) ?? []).length + (ps.match(/<path /g) ?? []).length;
assert.ok(marks <= MAX_POINTS + 50, `pause marks ${marks}`);
assert.equal((ps.match(/<path /g) ?? []).length, 50, "every Full GC triangle is kept");
const hs = heapSvg(series, "uptime", null, "Heap");
const coords = (hs.match(/points="([^"]*)"/g) ?? []).join(" ").split(" ").length;
assert.ok(coords <= MAX_POINTS + 10, `heap polyline points ${coords}`);
assert.ok(hs.includes('stroke="#4a3aa7"') && hs.includes('stroke="#eb6834"'));
assert.ok(ps.length < 400_000 && hs.length < 200_000, `sizes ${ps.length} / ${hs.length}`);
assert.equal(pauseSvg({ ...series, pause: f64(() => 0) }, "uptime", null, "x"), "");
const hist = histogramSvg([{ lo_ms: 1, hi_ms: 2, count: 3 }, { lo_ms: 2, hi_ms: 5, count: 9 }], "Hist <&>");
assert.ok(hist.includes('aria-label="Hist &lt;&amp;&gt;"') && hist.includes(">9<"));
console.log("gc-svg: all assertions passed");
