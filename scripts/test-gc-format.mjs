#!/usr/bin/env node
// Unit tests for web/src/gcFormat.ts (feat-074).
import assert from "node:assert/strict";
import { collectorLabel, formatKb, formatMs, formatPct, formatRate } from "../web/src/gcFormat.ts";

assert.equal(formatKb(512), "512 KB");
assert.equal(formatKb(1536), "1.50 MB");
assert.equal(formatKb(64 * 1024), "64.0 MB");
assert.equal(formatKb(8 * 1024 * 1024), "8.00 GB");
assert.equal(formatKb(5592576), "5.33 GB");
assert.equal(formatKb(null), "—");

assert.equal(formatMs(0.335), "0.34 ms");
assert.equal(formatMs(0.005), "5 µs");
assert.equal(formatMs(0), "0.00 ms");
assert.equal(formatMs(4.987), "5.0 ms");
assert.equal(formatMs(182.4), "182 ms");
assert.equal(formatMs(1420), "1.42 s");
assert.equal(formatMs(10267.3857), "10.3 s");
assert.equal(formatMs(242_000), "4m 02s");
assert.equal(formatMs(6 * 3600_000 + 12 * 60_000), "6h 12m");
assert.equal(formatMs(334_711_118), "3d 20h");
assert.equal(formatMs(NaN), "—");

assert.equal(formatPct(97.84), "97.8%");
assert.equal(formatPct(99.951), "99.95%");
assert.equal(formatPct(100), "100%");
assert.equal(formatRate(0.5), "512 KB/s");
assert.equal(formatRate(42.25), "42.3 MB/s");
assert.equal(formatRate(2048), "2.00 GB/s");
assert.equal(collectorLabel("g1"), "G1");
assert.equal(collectorLabel("zgc"), "ZGC");
console.log("gc-format: all assertions passed");
