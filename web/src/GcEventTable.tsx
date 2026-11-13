import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { gcEvents } from "./analyzer";
import { formatKb, formatMs, formatWall } from "./gcFormat";
import type { TimeAxis } from "./gcCharts";
import { GC_KINDS, type CauseStat, type EventQuery, type EventRow, type GcKind, type MemChange } from "./gcTypes";
import { useI18n, type MessageKey } from "./i18n";
import type { TableQuery } from "./gcTable";

/** Fixed row height (px) for virtual scrolling. */
const ROW_H = 30;
const PAGE = 200;
const VIEWPORT_ROWS = 16;
const OVERSCAN = 10;

interface Props {
  group: number;
  /** Bumps when the session / grouping changes (invalidates cached pages). */
  version: unknown;
  range: [number, number] | null;
  causes: CauseStat[];
  /** Kinds present in the group (only these get a filter chip). */
  kinds: GcKind[];
  axis: TimeAxis;
  query: TableQuery;
  onQuery: (q: TableQuery) => void;
  selectedRow: number | null;
  onSelectRow: (row: number) => void;
  /** Table position to scroll to (from a chart click). */
  scrollTo: { position: number; nonce: number } | null;
  names: string[];
}

function toEventQuery(q: TableQuery, range: [number, number] | null, offset: number): EventQuery {
  const min = Number(q.minDurationMs);
  return {
    offset,
    limit: PAGE,
    range,
    kinds: q.kinds.length ? q.kinds : null,
    cause: q.cause || null,
    min_duration_ms: q.minDurationMs !== "" && Number.isFinite(min) ? min : null,
    pauses_only: q.pausesOnly,
    sort: q.sort,
    desc: q.desc,
  };
}

function memCell(m: MemChange | null): string {
  if (!m) return "—";
  const cap = m.capacity_after_kb != null ? ` (${formatKb(m.capacity_after_kb)})` : "";
  return `${formatKb(m.before_kb)} → ${formatKb(m.after_kb)}${cap}`;
}

export default function GcEventTable({
  group,
  version,
  range,
  causes,
  kinds,
  axis,
  query,
  onQuery,
  selectedRow,
  onSelectRow,
  scrollTo,
  names,
}: Props) {
  const { t } = useI18n();
  const [total, setTotal] = useState(0);
  const [pages, setPages] = useState<Map<number, EventRow[]>>(new Map());
  const [scrollTop, setScrollTop] = useState(0);
  const viewport = useRef<HTMLDivElement>(null);
  const loading = useRef<Set<number>>(new Set());
  const queryKey = JSON.stringify([group, range, query, version]);

  // New query: drop cached pages and fetch the first page (for the total).
  useEffect(() => {
    let cancelled = false;
    loading.current = new Set([0]);
    setPages(new Map());
    void gcEvents(group, toEventQuery(query, range, 0)).then((page) => {
      if (cancelled) return;
      setTotal(page.total);
      setPages(new Map([[0, page.rows]]));
      loading.current.delete(0);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [queryKey]);

  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const last = Math.min(total, Math.ceil(scrollTop / ROW_H) + VIEWPORT_ROWS + OVERSCAN);

  // Fetch any page overlapping the visible window.
  useEffect(() => {
    if (total === 0) return;
    let cancelled = false;
    for (let p = Math.floor(first / PAGE); p <= Math.floor(Math.max(last - 1, 0) / PAGE); p++) {
      if (pages.has(p) || loading.current.has(p)) continue;
      loading.current.add(p);
      const pageNo = p;
      void gcEvents(group, toEventQuery(query, range, pageNo * PAGE)).then((page) => {
        loading.current.delete(pageNo);
        if (!cancelled) setPages((prev) => new Map(prev).set(pageNo, page.rows));
      });
    }
    return () => {
      cancelled = true;
    };
  }, [first, last, total, pages, group, query, range]);

  // A chart click resets the filters, so wait until the new total covers the target row.
  const pendingScroll = useRef<number | null>(null);
  useEffect(() => {
    if (scrollTo) pendingScroll.current = scrollTo.position;
  }, [scrollTo]);
  useEffect(() => {
    const pos = pendingScroll.current;
    if (pos == null || !viewport.current || pos >= total) return;
    viewport.current.scrollTop = Math.max(0, (pos - 3) * ROW_H);
    pendingScroll.current = null;
  }, [scrollTo, total]);

  const rowAt = useCallback((i: number) => pages.get(Math.floor(i / PAGE))?.[i % PAGE], [pages]);

  const visible = useMemo(() => {
    const out: { i: number; row: EventRow | undefined }[] = [];
    for (let i = first; i < last; i++) out.push({ i, row: rowAt(i) });
    return out;
  }, [first, last, rowAt]);

  const timeLabel = (r: EventRow) =>
    axis === "wall" && r.wall_ms != null ? formatWall(r.wall_ms) : formatMs(r.time_ms);

  const sortBy = (col: "time" | "duration") =>
    onQuery({ ...query, sort: col, desc: query.sort === col ? !query.desc : col === "duration" });
  const sortMark = (col: "time" | "duration") => (query.sort === col ? (query.desc ? " ↓" : " ↑") : "");

  const flagsOf = (r: EventRow) =>
    [
      r.flags.evacuation_failure && t("gc.flag.evacuationFailure"),
      r.flags.promotion_failed && t("gc.flag.promotionFailed"),
      r.flags.concurrent_mode_failure && t("gc.flag.concurrentModeFailure"),
      r.flags.allocation_stall && t("gc.flag.allocationStall"),
      r.flags.degenerated && t("gc.flag.degenerated"),
    ]
      .filter(Boolean)
      .join(", ");

  return (
    <div className="gc-events" data-testid="gc-events">
      <div className="gc-event-filters" role="group" aria-label={t("gc.events.filters")}>
        <div className="gc-kind-filter">
          {GC_KINDS.filter((k) => kinds.includes(k)).map((k) => {
            const on = query.kinds.includes(k);
            return (
              <button
                key={k}
                type="button"
                className={`segmented-option${on ? " active" : ""}`}
                aria-pressed={on}
                data-testid={`gc-filter-kind-${k}`}
                onClick={() =>
                  onQuery({ ...query, kinds: on ? query.kinds.filter((x) => x !== k) : [...query.kinds, k] })
                }
              >
                {t(`gc.kind.${k}` as MessageKey)}
              </button>
            );
          })}
        </div>
        <label className="gc-field">
          {t("gc.events.cause")}
          <select
            value={query.cause}
            data-testid="gc-filter-cause"
            onChange={(e) => onQuery({ ...query, cause: e.target.value })}
          >
            <option value="">{t("gc.events.allCauses")}</option>
            {causes.map((c) => (
              <option key={c.cause} value={c.cause}>
                {c.cause}
              </option>
            ))}
          </select>
        </label>
        <label className="gc-field">
          {t("gc.events.minDuration")}
          <input
            type="number"
            min={0}
            step="any"
            inputMode="decimal"
            value={query.minDurationMs}
            data-testid="gc-filter-min"
            onChange={(e) => onQuery({ ...query, minDurationMs: e.target.value })}
          />
        </label>
        <label className="gc-toggle">
          <input
            type="checkbox"
            checked={query.pausesOnly}
            data-testid="gc-filter-pauses"
            onChange={(e) => onQuery({ ...query, pausesOnly: e.target.checked })}
          />
          {t("gc.events.pausesOnly")}
        </label>
        <span className="gc-hint" data-testid="gc-events-count">
          {t("gc.events.count", { count: total })}
        </span>
      </div>

      <div className="gc-event-scroll">
        <div className="gc-event-grid gc-event-head" role="row">
          <button type="button" className="gc-sort" data-testid="gc-sort-time" onClick={() => sortBy("time")}>
            {t("gc.col.time")}
            {sortMark("time")}
          </button>
          <span>{t("gc.col.id")}</span>
          <span>{t("gc.col.event")}</span>
          <span>{t("gc.col.cause")}</span>
          <button type="button" className="gc-sort num" data-testid="gc-sort-duration" onClick={() => sortBy("duration")}>
            {t("gc.col.duration")}
            {sortMark("duration")}
          </button>
          <span>{t("gc.col.heap")}</span>
          <span>{t("gc.col.old")}</span>
          <span className="num">{t("gc.col.cpu")}</span>
          <span>{t("gc.col.source")}</span>
        </div>
        <div
          ref={viewport}
          className="gc-event-viewport"
          style={{ height: VIEWPORT_ROWS * ROW_H }}
          onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
          role="rowgroup"
          data-testid="gc-event-viewport"
        >
          {total === 0 ? (
            <p className="empty">{t("gc.events.empty")}</p>
          ) : (
            <div className="gc-event-spacer" style={{ height: total * ROW_H }}>
              {visible.map(({ i, row }) => (
                <div
                  key={i}
                  role="row"
                  tabIndex={row ? 0 : -1}
                  className={`gc-event-grid gc-event-row${row && row.index === selectedRow ? " is-selected" : ""}${
                    row && !row.is_pause ? " is-concurrent" : ""
                  }`}
                  style={{ transform: `translateY(${i * ROW_H}px)` }}
                  data-testid={row ? `gc-event-row-${row.index}` : undefined}
                  onClick={() => row && onSelectRow(row.index)}
                  onKeyDown={(e) => {
                    if (row && (e.key === "Enter" || e.key === " ")) {
                      e.preventDefault();
                      onSelectRow(row.index);
                    }
                  }}
                >
                  {row ? (
                    <>
                      <span className="mono">{timeLabel(row)}</span>
                      <span className="mono">{row.gc_id ?? "—"}</span>
                      <span title={flagsOf(row) || undefined}>
                        {row.name}
                        {flagsOf(row) && <span className="gc-flag"> ⚠ {flagsOf(row)}</span>}
                      </span>
                      <span>{row.cause ?? "—"}</span>
                      <span className="num mono">{formatMs(row.duration_ms)}</span>
                      <span className="mono">{memCell(row.heap)}</span>
                      <span className="mono">{memCell(row.old)}</span>
                      <span className="num mono">
                        {row.cpu ? `${formatMs(row.cpu.user_ms)} / ${formatMs(row.cpu.sys_ms)} / ${formatMs(row.cpu.real_ms)}` : "—"}
                      </span>
                      <span className="mono gc-source" title={`${names[row.file_index] ?? ""}:${row.line}`}>
                        {names[row.file_index] ?? "?"}:{row.line}
                      </span>
                    </>
                  ) : (
                    <span className="gc-event-loading">…</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
