// Mirror of the GC report types serialized from Rust (`src/gc/*.rs`, feat-074).
// Keep in sync with `src/gc/model.rs`, `stats.rs`, `stitch.rs`, `report.rs`, `findings.rs`.

export type GcLogFormat = "unified" | "legacy" | "unknown";

export type Collector =
  | "g1"
  | "parallel"
  | "serial"
  | "cms"
  | "zgc"
  | "shenandoah"
  | "unknown";

/** Code order of the `kind` series column (`GcKind::ALL`). */
export const GC_KINDS = [
  "young",
  "mixed",
  "full",
  "remark",
  "cleanup",
  "initial-mark",
  "other-pause",
  "concurrent",
  "stall",
] as const;

export type GcKind = (typeof GC_KINDS)[number];

export type GroupingMode = "auto" | "merge" | "split";

export interface MemChange {
  before_kb: number;
  after_kb: number;
  capacity_before_kb: number | null;
  capacity_after_kb: number | null;
}

export interface CpuTimes {
  user_ms: number;
  sys_ms: number;
  real_ms: number;
}

export interface EventFlags {
  evacuation_failure: boolean;
  promotion_failed: boolean;
  concurrent_mode_failure: boolean;
  allocation_stall: boolean;
  degenerated: boolean;
}

export interface GcEvent {
  gc_id: number | null;
  time_ms: number;
  uptime_ms: number | null;
  wall_ms: number | null;
  kind: GcKind;
  is_pause: boolean;
  name: string;
  cause: string | null;
  duration_ms: number;
  heap: MemChange | null;
  young: MemChange | null;
  old: MemChange | null;
  humongous: MemChange | null;
  metaspace: MemChange | null;
  cpu: CpuTimes | null;
  flags: EventFlags;
  file_index: number;
  line: number;
}

export interface JvmFact {
  key: string;
  value: string;
}

export interface JvmInfo {
  version: string | null;
  heap_max_kb: number | null;
  heap_initial_kb: number | null;
  region_size_kb: number | null;
  pause_target_ms: number | null;
  command_line: string | null;
  facts: JvmFact[];
}

export interface Percentiles {
  mean: number;
  p50: number;
  p90: number;
  p95: number;
  p99: number;
  max: number;
}

export interface KindStat {
  kind: GcKind;
  count: number;
  total_ms: number;
  max_ms: number;
}

export interface CauseStat {
  cause: string;
  count: number;
  total_pause_ms: number;
  max_pause_ms: number;
}

export interface RateStats {
  avg_mb_per_s: number;
  peak_mb_per_s: number;
}

export interface HistogramBucket {
  lo_ms: number;
  hi_ms: number | null;
  count: number;
}

export interface GcStats {
  range: [number, number] | null;
  start_ms: number;
  end_ms: number;
  span_ms: number;
  event_count: number;
  pause_count: number;
  total_pause_ms: number;
  throughput_pct: number | null;
  pause: Percentiles | null;
  mean_pause_interval_ms: number | null;
  kinds: KindStat[];
  concurrent_ms: number;
  concurrent_pct: number | null;
  allocation: RateStats | null;
  promotion: RateStats | null;
  heap_after_p50_kb: number | null;
  heap_after_max_kb: number | null;
  heap_after_peak_pct: number | null;
  metaspace_peak_kb: number | null;
  causes: CauseStat[];
  histogram: HistogramBucket[];
}

export type Severity = "critical" | "warning" | "info";

export interface GcFinding {
  rule: string;
  severity: Severity;
  detail: string;
  anchors: [number, number][];
}

export type NoteKind = "gap" | "overlap" | "duplicate" | "missing-start";

export interface GroupNote {
  kind: NoteKind;
  file_index: number;
  detail: string;
}

export interface FileFingerprint {
  name: string;
  format: GcLogFormat;
  collector: Collector;
  version: string | null;
  pid: number | null;
  first_gc_id: number | null;
  last_gc_id: number | null;
  first_uptime_ms: number | null;
  last_uptime_ms: number | null;
  first_wall_ms: number | null;
  last_wall_ms: number | null;
  jvm_start_ms: number | null;
  event_count: number;
  line_count: number;
  unparsed_count: number;
  unparsed_samples: [number, string][];
}

export interface GroupReport {
  index: number;
  name: string;
  files: number[];
  format: GcLogFormat;
  collector: Collector;
  jvm: JvmInfo;
  event_count: number;
  missing_timestamps: boolean;
  has_uptime: boolean;
  has_wall: boolean;
  wall_offset_ms: number | null;
  stats: GcStats;
  findings: GcFinding[];
  notes: GroupNote[];
  unparsed_count: number;
  unparsed_samples: [number, string][];
}

export interface GcReport {
  mode: GroupingMode;
  files: FileFingerprint[];
  groups: GroupReport[];
}

/** Columnar chart data for one group (`SERIES_COLUMNS` + derived columns). */
export type GcSeries = Record<string, Float64Array>;

/** Flag bits of the `flags` column (`src/gc/report.rs`). */
export const FLAG_EVACUATION_FAILURE = 1;
export const FLAG_PROMOTION_FAILED = 2;
export const FLAG_CONCURRENT_MODE_FAILURE = 4;
export const FLAG_ALLOCATION_STALL = 8;
export const FLAG_DEGENERATED = 16;

export interface EventQuery {
  offset?: number;
  limit?: number;
  range?: [number, number] | null;
  kinds?: GcKind[] | null;
  cause?: string | null;
  min_duration_ms?: number | null;
  pauses_only?: boolean;
  sort?: "time" | "duration";
  desc?: boolean;
}

export interface EventRow extends GcEvent {
  index: number;
}

export interface EventPage {
  total: number;
  offset: number;
  rows: EventRow[];
}

export interface InputKinds {
  thread_dump: boolean;
  gc_log: boolean;
}
