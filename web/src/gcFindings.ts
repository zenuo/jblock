/**
 * Render GC findings (feat-072) from the Rust `detail` evidence (`key=value;…`)
 * into localized title / body / suggestion text. Pure; Node-testable with a
 * stub translator.
 */

import { formatKb, formatMs, formatPct } from "./gcFormat.ts";
import type { GcFinding } from "./gcTypes.ts";

/** Minimal translator surface (`useI18n().t`). */
export type Translate = (key: string, params?: Record<string, string | number>) => string;

export interface FindingText {
  title: string;
  body: string;
  fix: string;
}

/** `count=3;max_ms=12.5;causes=a:1,b:2` → `{ count: "3", … }` (values may contain `:` and `,`). */
export function parseDetail(detail: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of detail.split(";")) {
    const eq = part.indexOf("=");
    if (eq > 0) out[part.slice(0, eq)] = part.slice(eq + 1);
  }
  return out;
}

const num = (v: string | undefined) => (v == null || v === "none" ? NaN : Number(v));

/** `a:1,b:2` → `a ×1, b ×2`. */
function formatCauses(raw: string | undefined): string {
  if (!raw) return "—";
  return raw
    .split(",")
    .map((pair) => {
      const i = pair.lastIndexOf(":");
      return i > 0 ? `${pair.slice(0, i)} ×${pair.slice(i + 1)}` : pair;
    })
    .join(", ");
}

/** Localized text for one finding. */
export function findingText(f: GcFinding, t: Translate): FindingText {
  const d = parseDetail(f.detail);
  const key = (part: "title" | "body" | "fix") => `gc.rule.${f.rule}.${part}`;
  let params: Record<string, string | number> = {};
  let extra = "";
  switch (f.rule) {
    case "GC01":
      params = { count: d.count, max: formatMs(num(d.max_ms)), causes: formatCauses(d.causes) };
      break;
    case "GC02":
      params = { runs: d.runs, longest: d.longest, max: formatMs(num(d.max_ms)) };
      if (num(d.starved_runs) > 0) extra = t("gc.rule.GC02.starved", { starved: d.starved_runs });
      break;
    case "GC03":
      params = { count: d.count, ratio: formatPct(num(d.ratio_pct)) };
      if (d.hourly === "1") extra = t("gc.rule.GC03.hourly");
      break;
    case "GC04":
      params = { count: d.count };
      break;
    case "GC05":
      params = { promo: d.promotion_failed, cmf: d.concurrent_mode_failure };
      break;
    case "GC06":
      params = { count: d.count, causePct: formatPct(num(d.cause_pct)), peakVsOld: formatPct(num(d.peak_vs_old_pct)) };
      break;
    case "GC07":
      params = {
        max: formatMs(num(d.max_ms)),
        p99: formatMs(num(d.p99_ms)),
        target: d.target_ms === "none" ? t("gc.rule.noTarget") : formatMs(num(d.target_ms)),
        countOver: d.count_over,
      };
      break;
    case "GC08":
      params = {
        window: formatPct(num(d.window_pct)),
        windowS: d.window_s,
        global: formatPct(num(d.global_pct)),
        goal: d.goal_pct === "none" ? t("gc.rule.noTarget") : formatPct(num(d.goal_pct)),
      };
      break;
    case "GC09":
      params = { slope: formatKb(num(d.slope_mb_per_h) * 1024), r2: d.r2, points: d.points };
      if (d.hours_to_full && d.hours_to_full !== "none") extra += t("gc.rule.GC09.hours", { hours: d.hours_to_full });
      if (d.old_gc === "0") extra += t("gc.rule.GC09.noOldGc");
      break;
    case "GC10":
      params = { peak: formatPct(num(d.peak_pct)), countOver: d.count_over };
      break;
    case "GC11":
      params = { slope: formatKb(num(d.slope_mib_s) * 1024), threshold: d.metadata_threshold };
      break;
    case "GC12":
      params = { inverted: d.inverted, sysHeavy: d.sys_heavy };
      break;
    case "GC13":
      params = { interval: formatMs(num(d.mean_interval_ms)), count: d.count };
      break;
    case "GC14":
      params = { stalls: d.stalls, perMin: d.stalls_per_min, degenerated: d.degenerated };
      break;
    default:
      return { title: f.rule, body: f.detail, fix: "" };
  }
  const bodyKey = f.rule === "GC07" && d.target_ms === "none" ? "gc.rule.GC07.bodyNoTarget" : key("body");
  return { title: t(key("title"), params), body: t(bodyKey, params) + extra, fix: t(key("fix"), params) };
}

/** Zoom window for Locate: the anchors' hull plus 10% on each side. */
export function locateRange(f: GcFinding): [number, number] | null {
  if (f.anchors.length === 0) return null;
  const lo = Math.min(...f.anchors.map((a) => a[0]));
  const hi = Math.max(...f.anchors.map((a) => a[1]));
  const pad = Math.max((hi - lo) * 0.1, 500);
  return [Math.max(lo - pad, 0), hi + pad];
}
