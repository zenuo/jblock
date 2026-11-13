#!/usr/bin/env node
// Unit tests for web/src/gcFindings.ts (feat-072): detail parsing, rendering, locate range.
import assert from "node:assert/strict";
import { findingText, locateRange, parseDetail } from "../web/src/gcFindings.ts";
import { GC_MESSAGES } from "../web/src/i18n/gcMessages.ts";

const en = GC_MESSAGES.en;
const t = (key, params = {}) => (en[key] ?? `!${key}`).replace(/\{(\w+)\}/g, (_, k) => String(params[k] ?? `{${k}}`));

assert.deepEqual(parseDetail("count=2;max_ms=12.5;causes=G1 Compaction Pause:1,Allocation Failure:1"), {
  count: "2",
  max_ms: "12.5",
  causes: "G1 Compaction Pause:1,Allocation Failure:1",
});

const gc01 = findingText({ rule: "GC01", severity: "warning", detail: "count=2;max_ms=1420;causes=G1 Compaction Pause:2", anchors: [] }, t);
assert.equal(gc01.title, "Full GC occurred");
assert.match(gc01.body, /^2 Full GC\(s\), longest 1\.42 s\. Causes: G1 Compaction Pause ×2\./);

const gc02 = findingText({ rule: "GC02", severity: "critical", detail: "runs=1;longest=6;full=6;starved_runs=1;max_ms=10267.3857", anchors: [] }, t);
assert.match(gc02.body, /up to 6 in a row, longest pause 10\.3 s/);
assert.match(gc02.body, /less than 2% of the old generation free/);

const gc07 = findingText({ rule: "GC07", severity: "warning", detail: "max_ms=1500;p99_ms=350;target_ms=none;count_over=2", anchors: [] }, t);
assert.equal(gc07.body, "Longest pause 1.50 s, P99 350 ms; 2 pause(s) longer than 1 s (this collector has no pause target).");

const gc07t = findingText({ rule: "GC07", severity: "warning", detail: "max_ms=350;p99_ms=250;target_ms=200;count_over=3", anchors: [] }, t);
assert.equal(gc07t.body, "Longest pause 350 ms, P99 250 ms (pause target 200 ms); 3 pause(s) above the bar.");

const gc09 = findingText({ rule: "GC09", severity: "warning", detail: "slope_mb_per_h=600;pct_per_s=0.01;r2=0.98;hours_to_full=10;points=108;old_gc=0", anchors: [] }, t);
assert.match(gc09.body, /grows by 600 MB per hour/);
assert.match(gc09.body, /full in about 10 h/);
assert.match(gc09.body, /No old-generation collection/);

// Every rule renders without leftover placeholders or missing keys in every locale.
const details = {
  GC01: "count=1;max_ms=5;causes=x:1", GC02: "runs=1;longest=3;full=3;starved_runs=0;max_ms=9", GC03: "count=3;ratio_pct=50;hourly=1",
  GC04: "count=9", GC05: "promotion_failed=1;concurrent_mode_failure=2", GC06: "count=3;cause_pct=12;peak_vs_old_pct=60",
  GC07: "max_ms=300;p99_ms=250;target_ms=none;count_over=4", GC08: "window_pct=12;window_s=60;global_pct=3;goal_pct=7.69",
  GC09: "slope_mb_per_h=10;pct_per_s=0.1;r2=0.9;hours_to_full=none;points=9;old_gc=1", GC10: "peak_pct=90;count_over=2",
  GC11: "slope_mib_s=1;metadata_threshold=3", GC12: "inverted=1;sys_heavy=0", GC13: "mean_interval_ms=500;count=30",
  GC14: "stalls=12;stalls_per_min=12;degenerated=0",
};
for (const [locale, cat] of Object.entries(GC_MESSAGES)) {
  const tl = (key, params = {}) => {
    assert.ok(key in cat, `${locale}: missing ${key}`);
    return cat[key].replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m));
  };
  for (const [rule, detail] of Object.entries(details)) {
    const text = findingText({ rule, severity: "warning", detail, anchors: [] }, tl);
    for (const part of [text.title, text.body, text.fix]) {
      assert.ok(part.length > 0, `${locale} ${rule} empty`);
      assert.ok(!/\{\w+\}/.test(part), `${locale} ${rule}: unfilled placeholder in "${part}"`);
      assert.ok(!part.includes("undefined") && !part.includes("NaN"), `${locale} ${rule}: bad value in "${part}"`);
    }
  }
}

assert.deepEqual(locateRange({ rule: "GC04", severity: "critical", detail: "", anchors: [[10_000, 10_100], [20_000, 20_050]] }), [8_995, 21_055]); // hull 10050 ms ± 10%
assert.deepEqual(locateRange({ rule: "GC04", severity: "critical", detail: "", anchors: [[100, 101]] }), [0, 601]);
assert.equal(locateRange({ rule: "GC04", severity: "critical", detail: "", anchors: [] }), null);
console.log("gc-findings: all assertions passed");
