/** Number formatting for the GC views (feat-074). Pure functions; Node-testable. */

/** KiB → `512 KB`, `1.5 MB`, `7.84 GB`. */
export function formatKb(kb: number | null | undefined): string {
  if (kb == null || !Number.isFinite(kb)) return "—";
  const units = ["KB", "MB", "GB", "TB"];
  let v = kb;
  let u = 0;
  while (Math.abs(v) >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  const digits = u === 0 || Math.abs(v) >= 100 ? 0 : Math.abs(v) >= 10 ? 1 : 2;
  return `${v.toFixed(digits)} ${units[u]}`;
}

/** Milliseconds → `5 µs`, `0.12 ms`, `45 ms`, `1.42 s`, `4m 02s`, `6h 12m`, `3d 4h`. */
export function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "—";
  const a = Math.abs(ms);
  // ZGC / Shenandoah pauses are often a few microseconds.
  if (a > 0 && a < 0.1) return `${Math.round(ms * 1000)} µs`;
  if (a < 1) return `${ms.toFixed(2)} ms`;
  if (a < 10) return `${ms.toFixed(1)} ms`;
  if (a < 1000) return `${Math.round(ms)} ms`;
  if (a < 60_000) return `${(ms / 1000).toFixed(a < 10_000 ? 2 : 1)} s`;
  const totalSec = Math.round(a / 1000);
  const sign = ms < 0 ? "-" : "";
  const d = Math.floor(totalSec / 86_400);
  const h = Math.floor((totalSec % 86_400) / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (d > 0) return `${sign}${d}d ${h}h`;
  if (h > 0) return `${sign}${h}h ${String(m).padStart(2, "0")}m`;
  return `${sign}${m}m ${String(s).padStart(2, "0")}s`;
}

/** Percentage with adaptive precision: `97.8%`, `99.95%`, `100%`. */
export function formatPct(pct: number | null | undefined): string {
  if (pct == null || !Number.isFinite(pct)) return "—";
  if (pct >= 100) return "100%";
  if (pct >= 99) return `${pct.toFixed(2)}%`;
  return `${pct.toFixed(1)}%`;
}

/** MB/s with sensible precision. */
export function formatRate(mbPerS: number | null | undefined): string {
  if (mbPerS == null || !Number.isFinite(mbPerS)) return "—";
  if (mbPerS >= 1024) return `${(mbPerS / 1024).toFixed(2)} GB/s`;
  if (mbPerS >= 100) return `${Math.round(mbPerS)} MB/s`;
  if (mbPerS >= 1) return `${mbPerS.toFixed(1)} MB/s`;
  return `${(mbPerS * 1024).toFixed(0)} KB/s`;
}

/** Epoch ms → local `YYYY-MM-DD HH:MM:SS`. */
export function formatWall(epochMs: number | null | undefined): string {
  if (epochMs == null || !Number.isFinite(epochMs)) return "—";
  const d = new Date(epochMs);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/** Collector enum → display label. */
export function collectorLabel(c: string): string {
  const labels: Record<string, string> = {
    g1: "G1",
    parallel: "Parallel",
    serial: "Serial",
    cms: "CMS",
    zgc: "ZGC",
    shenandoah: "Shenandoah",
    unknown: "?",
  };
  return labels[c] ?? c;
}
