#!/usr/bin/env node
// Unit tests for web/src/gcCharts.ts (feat-075) on a hand-built series.
import assert from "node:assert/strict";
import {
  buildHeapData,
  buildPauseData,
  concurrentBands,
  fromX,
  fullGcXs,
  lowerBound,
  pauseBucket,
  preferLogScale,
  toX,
} from "../web/src/gcCharts.ts";
import { GC_KINDS } from "../web/src/gcTypes.ts";

const K = (k) => GC_KINDS.indexOf(k);
const NaN_ = Number.NaN;
// young @1000 (10ms, 100→20 MB), concurrent @1500 (+400), full @3000 (50ms, 90→30 MB, old 25), remark @2000 (no heap)
const series = {
  time_ms: Float64Array.from([1000, 1500, 2000, 3000]),
  duration_ms: Float64Array.from([10, 400, 2, 50]),
  kind: Float64Array.from([K("young"), K("concurrent"), K("remark"), K("full")]),
  pause: Float64Array.from([1, 0, 1, 1]),
  heap_before_kb: Float64Array.from([100 * 1024, NaN_, NaN_, 90 * 1024]),
  heap_after_kb: Float64Array.from([20 * 1024, NaN_, NaN_, 30 * 1024]),
  heap_capacity_kb: Float64Array.from([256 * 1024, NaN_, NaN_, 256 * 1024]),
  old_after_kb: Float64Array.from([NaN_, NaN_, NaN_, 25 * 1024]),
};

assert.equal(toX(1500, "uptime", null), 1.5);
assert.equal(toX(1500, "wall", 1_700_000_000_000), 1_700_000_001.5);
assert.equal(fromX(1_700_000_001.5, "wall", 1_700_000_000_000), 1500);
assert.equal(fromX(2, "wall", null), 2000, "wall without offset falls back to uptime");

const p = buildPauseData(series, "uptime", null);
assert.deepEqual(p.data[0], [1, 2, 3]);
assert.deepEqual(p.data[1], [10, 2, 50]);
assert.deepEqual(p.rows, [0, 2, 3]);
assert.deepEqual(p.buckets, ["young", "other", "full"]);
assert.equal(pauseBucket("mixed"), "mixed");
assert.equal(pauseBucket("cleanup"), "other");

const h = buildHeapData(series, "uptime", null);
assert.deepEqual(h.data[0], [1, 1.01, 3, 3.05]);
assert.deepEqual(h.data[1], [100, 20, 90, 30]);
assert.deepEqual(h.data[2], [null, null, null, 25]);
assert.deepEqual(h.data[3], [256, 256, 256, 256]);
assert.deepEqual(h.rows, [0, 0, 3, 3]);
assert.equal(h.hasOld, true);
assert.equal(h.hasCapacity, true);

assert.deepEqual(concurrentBands(series, "uptime", null), [[1.5, 1.9]]);
assert.deepEqual(fullGcXs(series, "uptime", null), [3]);
assert.equal(lowerBound([1, 2, 3, 5], 4), 3);
assert.equal(lowerBound([1, 2, 3], 0), 0);
assert.equal(lowerBound([1, 2, 3], 9), 3);
assert.equal(preferLogScale([0.1, 0.2, 50]), true);
assert.equal(preferLogScale([1, 2, 50]), false);
console.log("gc-charts: all assertions passed");

// feat-076 builders.
import { buildMetaspaceData, buildRateData, bucketLabel } from "../web/src/gcCharts.ts";
const s2 = {
  ...series,
  alloc_rate_mb_s: Float64Array.from([NaN_, NaN_, NaN_, 42]),
  promotion_rate_mb_s: Float64Array.from([NaN_, NaN_, NaN_, NaN_]),
  metaspace_after_kb: Float64Array.from([2048, NaN_, NaN_, 3072]),
  metaspace_capacity_kb: Float64Array.from([4096, NaN_, NaN_, 4096]),
};
const r = buildRateData(s2, "uptime", null);
assert.deepEqual(r.data, [[3], [42], [null]]);
assert.equal(r.hasPromotion, false);
const m = buildMetaspaceData(s2, "uptime", null);
assert.deepEqual(m.data, [[1.01, 3.05], [2, 3], [4, 4]]);
assert.equal(m.hasCommitted, true);
assert.equal(bucketLabel(0.1, 0.2), "0.1–0.2 ms");
assert.equal(bucketLabel(500, 1000), "500 ms–1 s");
assert.equal(bucketLabel(1000, 2000), "1–2 s");
assert.equal(bucketLabel(5000, null), "≥ 5 s");
console.log("gc-charts (076): all assertions passed");

// feat-078: worst value per comparison column.
import { worstRow } from "../web/src/gcCharts.ts";
assert.equal(worstRow([99.5, 97.1, 98.0], "low"), 1);
assert.equal(worstRow([10, 250, 30], "high"), 1);
assert.equal(worstRow([null, 5, 3], "high"), 1);
assert.equal(worstRow([4, 4], "high"), null, "all equal → nothing is worst");
assert.equal(worstRow([7, null], "high"), null, "single value → nothing to compare");
console.log("gc-charts (078): all assertions passed");
