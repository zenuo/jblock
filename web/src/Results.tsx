import { useEffect, useMemo, useState } from "react";
import {
  aggregateContention,
  applySortClick,
  buildFindings,
  clusterByStack,
  isJvmNoise,
  sortThreads,
  threadDomId,
  threadHasWaitingOn,
  type Finding,
  type ThreadSortKey,
  type ThreadSortSpec,
} from "./analysisUi";
import FlameGraph from "./FlameGraph";
import { useI18n } from "./i18n";
import PatternLegendModal from "./PatternLegendModal";
import ReportNav from "./ReportNav";
import {
  parseSectionId,
  readNavCollapsed,
  reportSections,
  sectionDomId,
  storeNavCollapsed,
  type ReportSectionId,
} from "./reportNav";
import type { Analysis, ThreadInfo } from "./types";
import { tokenizeStackFrame } from "./stackFrame";

const STATE_COLORS: Record<string, string> = {
  RUNNABLE: "#22c55e",
  BLOCKED: "#ef4444",
  WAITING: "#f59e0b",
  TIMED_WAITING: "#eab308",
  NEW: "#38bdf8",
  TERMINATED: "#94a3b8",
};

type StateFilter = "ALL" | string;

interface Props {
  analysis: Analysis;
}

export default function Results({ analysis }: Props) {
  const { t } = useI18n();
  const findings = useMemo(() => buildFindings(analysis, t), [analysis, t]);
  const contentionGroups = useMemo(
    () => aggregateContention(analysis.blocked_edges),
    [analysis.blocked_edges],
  );

  const hasBlocked =
    (analysis.state_counts.find((s) => s.state === "BLOCKED")?.count ?? 0) > 0;
  const [stateFilter, setStateFilter] = useState<StateFilter>(
    hasBlocked ? "BLOCKED" : "ALL",
  );
  const [hideNoise, setHideNoise] = useState(true);
  const [sorts, setSorts] = useState<ThreadSortSpec[]>([
    { key: "name", dir: "asc" },
  ]);
  const [expandedLocks, setExpandedLocks] = useState<Set<string>>(new Set());
  const [expandedStacks, setExpandedStacks] = useState<Set<number>>(new Set());
  const [focusIndex, setFocusIndex] = useState<number | null>(null);
  const [legendFinding, setLegendFinding] = useState<Finding | null>(null);
  const [navCollapsed, setNavCollapsed] = useState(() => readNavCollapsed());
  const [activeSection, setActiveSection] = useState<ReportSectionId>("findings");

  useEffect(() => {
    setStateFilter(hasBlocked ? "BLOCKED" : "ALL");
    setExpandedLocks(new Set());
    setExpandedStacks(new Set());
    setFocusIndex(null);
  }, [analysis, hasBlocked]);

  const visibleThreads = useMemo(() => {
    let list = analysis.threads.map((th, index) => ({ t: th, index }));
    if (hideNoise) list = list.filter(({ t: th }) => !isJvmNoise(th.name));
    if (stateFilter !== "ALL") {
      list = list.filter(({ t: th }) => th.state === stateFilter);
    }
    return list;
  }, [analysis.threads, hideNoise, stateFilter]);

  const showWaitingOn = useMemo(
    () => visibleThreads.some(({ t }) => threadHasWaitingOn(t)),
    [visibleThreads],
  );

  const activeSorts = useMemo(() => {
    const next = showWaitingOn
      ? sorts
      : sorts.filter((s) => s.key !== "waiting");
    return next.length > 0 ? next : [{ key: "name" as const, dir: "asc" as const }];
  }, [sorts, showWaitingOn]);

  const filteredThreads = useMemo(() => {
    const sorted = sortThreads(
      visibleThreads.map(({ t }) => t),
      activeSorts,
    );
    return sorted.map((th) => {
      const index = analysis.threads.indexOf(th);
      return { t: th, index };
    });
  }, [visibleThreads, activeSorts, analysis.threads]);

  const clusters = useMemo(() => {
    const base = hideNoise
      ? analysis.threads.filter((th) => !isJvmNoise(th.name))
      : analysis.threads;
    const scoped =
      stateFilter === "ALL" ? base : base.filter((th) => th.state === stateFilter);
    return clusterByStack(scoped, 2).slice(0, 12);
  }, [analysis.threads, hideNoise, stateFilter]);

  const flameThreads = useMemo(
    () =>
      hideNoise
        ? analysis.threads.filter((th) => !isJvmNoise(th.name))
        : analysis.threads,
    [analysis.threads, hideNoise],
  );

  const sections = useMemo(
    () =>
      reportSections({
        hasDeadlocks: analysis.deadlocks.length > 0,
        hasClusters: clusters.length > 0,
      }),
    [analysis.deadlocks.length, clusters.length],
  );

  useEffect(() => {
    const els = sections
      .map((s) => document.getElementById(sectionDomId(s.id)))
      .filter((el): el is HTMLElement => el !== null);
    if (els.length === 0) return;
    const ratios = new Map<string, number>();
    const obs = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          ratios.set(entry.target.id, entry.isIntersecting ? entry.intersectionRatio : 0);
        }
        let bestId: string | null = null;
        let best = 0;
        for (const [id, ratio] of ratios) {
          if (ratio > best) {
            best = ratio;
            bestId = id;
          }
        }
        if (!bestId) return;
        const parsed = parseSectionId(bestId);
        if (parsed) setActiveSection(parsed);
      },
      { rootMargin: "-12% 0px -55% 0px", threshold: [0, 0.15, 0.35, 0.6, 1] },
    );
    for (const el of els) obs.observe(el);
    return () => obs.disconnect();
  }, [sections, analysis]);

  const toggleNav = () => {
    setNavCollapsed((prev) => {
      const next = !prev;
      storeNavCollapsed(next);
      return next;
    });
  };

  const jumpToSection = (domId: string) => {
    const parsed = parseSectionId(domId);
    if (parsed) setActiveSection(parsed);
    const el = document.getElementById(domId);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
    if (typeof history !== "undefined" && history.replaceState) {
      history.replaceState(null, "", `#${domId}`);
    }
  };

  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const noiseHidden = hideNoise
    ? analysis.threads.filter((th) => isJvmNoise(th.name)).length
    : 0;

  useEffect(() => {
    if (focusIndex === null) return;
    const el = document.getElementById(threadDomId(focusIndex));
    el?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [focusIndex, filteredThreads]);

  const focusThreadByName = (name: string) => {
    const idx = analysis.threads.findIndex((th) => th.name === name);
    if (idx < 0) return;
    const th = analysis.threads[idx];
    if (hideNoise && isJvmNoise(name)) setHideNoise(false);
    if (stateFilter !== "ALL" && th.state !== stateFilter) setStateFilter("ALL");
    setFocusIndex(idx);
  };

  const toggleLock = (lock: string) => {
    setExpandedLocks((prev) => {
      const next = new Set(prev);
      if (next.has(lock)) next.delete(lock);
      else next.add(lock);
      return next;
    });
  };

  const toggleStack = (index: number) => {
    setExpandedStacks((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const onSort = (key: ThreadSortKey, additive: boolean) => {
    setSorts((prev) => applySortClick(prev, key, additive));
  };

  const shownLabel =
    filteredThreads.length !== analysis.threads.length
      ? `${filteredThreads.length} / ${analysis.threads.length}`
      : String(filteredThreads.length);

  return (
    <div
      className={`results-shell${navCollapsed ? " is-nav-collapsed" : ""}`}
      data-testid="results-shell"
    >
      <ReportNav
        sections={sections}
        collapsed={navCollapsed}
        onToggle={toggleNav}
        activeId={activeSection}
        onNavigate={jumpToSection}
      />
      <main className="results" data-testid="results">
      <section
        className="panel findings"
        data-testid="findings"
        id={sectionDomId("findings")}
      >
        <div className="findings-header">
          <h2>{t("findings.title")}</h2>
          <div className="findings-header-meta">
            <span className="meta mono">
              {t("findings.meta", {
                count: analysis.total_threads,
                format: analysis.format,
              })}
            </span>
          </div>
        </div>
        {findings.length === 0 ? (
          <div className="finding finding-ok" data-testid="findings-ok">
            <div className="finding-row">
              <strong>
                <span className="finding-ok-mark" aria-hidden="true">
                  ✅
                </span>
                {t("findings.okTitle")}
              </strong>
            </div>
            <span className="mono">
              {t("findings.okDetail", {
                count: analysis.total_threads,
                format: analysis.format,
              })}
            </span>
          </div>
        ) : (
          <ul className="findings-list">
            {findings.map((f, i) => (
              <li key={i} className={`finding finding-${f.severity}`}>
                <div className="finding-row">
                  <strong>{f.title}</strong>
                  <button
                    type="button"
                    className="btn finding-legend-btn"
                    data-testid={`legend-btn-${f.kind}`}
                    onClick={() => setLegendFinding(f)}
                  >
                    {t("findings.legendBtn")}
                  </button>
                </div>
                <span className="mono">{f.detail}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {legendFinding && (
        <PatternLegendModal
          finding={legendFinding}
          onClose={() => setLegendFinding(null)}
        />
      )}

      {analysis.deadlocks.length > 0 && (
        <section
          className="panel deadlock-panel"
          data-testid="deadlocks"
          id={sectionDomId("deadlocks")}
        >
          <h2>{t("deadlocks.title", { count: analysis.deadlocks.length })}</h2>
          {analysis.deadlocks.map((d, i) => (
            <div key={i} className="deadlock-cycle">
              <span className="mono">
                {d.threads.map((name, j) => (
                  <span key={j}>
                    {j > 0 && " → "}
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => focusThreadByName(name)}
                    >
                      {name}
                    </button>
                  </span>
                ))}
                {" → "}
                <button
                  type="button"
                  className="linkish"
                  onClick={() => focusThreadByName(d.threads[0] ?? "")}
                >
                  {d.threads[0]}
                </button>
              </span>
              <ul>
                {d.edges.map((e, j) => (
                  <li key={j} className="mono">
                    <button
                      type="button"
                      className="linkish"
                      onClick={() => focusThreadByName(e.blocked_thread)}
                    >
                      {e.blocked_thread}
                    </button>{" "}
                    {t("deadlocks.waitsOn")} {e.lock} ({t("deadlocks.heldBy")}{" "}
                    {e.owner_thread ? (
                      <button
                        type="button"
                        className="linkish"
                        onClick={() => focusThreadByName(e.owner_thread!)}
                      >
                        {e.owner_thread}
                      </button>
                    ) : (
                      t("deadlocks.unknown")
                    )}
                    )
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </section>
      )}

      <section
        className="panel"
        data-testid="contention"
        id={sectionDomId("contention")}
      >
        <h2>
          {t("contention.title", { count: contentionGroups.length })}
        </h2>
        {contentionGroups.length === 0 ? (
          <p className="empty">{t("contention.empty")}</p>
        ) : (
          <ul className="contention-groups">
            {contentionGroups.map((g) => {
              const open = expandedLocks.has(g.lock);
              return (
                <li key={g.lock} className="contention-group">
                  <div className="contention-summary">
                    <button
                      type="button"
                      className="contention-toggle"
                      onClick={() => toggleLock(g.lock)}
                      aria-expanded={open}
                    >
                      <span className="badge">{g.waiters.length}</span>
                      <span className="mono lock-id">{g.lock}</span>
                      <span className="chevron">{open ? "▾" : "▸"}</span>
                    </button>
                    <span className="contention-owner">
                      {t("contention.heldBy")}{" "}
                      {g.owner_thread ? (
                        <button
                          type="button"
                          className="linkish"
                          onClick={() => focusThreadByName(g.owner_thread!)}
                        >
                          {g.owner_thread}
                        </button>
                      ) : (
                        t("contention.unknownOwner")
                      )}
                    </span>
                  </div>
                  {open && (
                    <ul className="waiter-list">
                      {g.waiters.map((w) => (
                        <li key={w}>
                          <button
                            type="button"
                            className="linkish"
                            onClick={() => focusThreadByName(w)}
                          >
                            {w}
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section
        className="panel states-panel"
        data-testid="thread-states"
        id={sectionDomId("states")}
      >
        <h2>{t("states.title")}</h2>
        <ul className="states">
          {analysis.state_counts.map((s) => (
            <li key={s.state}>
              <button
                type="button"
                className={`state-row${stateFilter === s.state ? " active" : ""}`}
                onClick={() =>
                  setStateFilter(stateFilter === s.state ? "ALL" : s.state)
                }
                aria-pressed={stateFilter === s.state}
              >
                <span className="state-name">{s.state}</span>
                <span className="bar-track">
                  <span
                    className="bar-fill"
                    style={{
                      width: `${(s.count / maxState) * 100}%`,
                      background: STATE_COLORS[s.state] ?? "#64748b",
                    }}
                  />
                </span>
                <span className="state-count">{s.count}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      {clusters.length > 0 && (
        <section
          className="panel"
          data-testid="clusters"
          id={sectionDomId("clusters")}
        >
          <h2>{t("clusters.title", { count: clusters.length })}</h2>
          <p className="empty">{t("clusters.blurb")}</p>
          <ul className="cluster-list">
            {clusters.map((c) => (
              <li key={c.signature} className="cluster-item">
                <div className="cluster-head">
                  <span className="badge">{c.count}</span>
                  <span
                    className="state-pill"
                    style={{ background: STATE_COLORS[c.state] ?? "#64748b" }}
                  >
                    {c.state}
                  </span>
                  <span className="mono">
                    {c.sample_names.join(", ")}
                    {c.count > c.sample_names.length ? ", …" : ""}
                  </span>
                </div>
                <ol className="stack-preview">
                  {c.frames.map((f, i) => (
                    <li key={i} className="mono">
                      <StackFrameView frame={f} />
                    </li>
                  ))}
                </ol>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="panel" data-testid="threads" id={sectionDomId("threads")}>
        <div className="threads-toolbar">
          <h2>{t("threads.title", { shown: shownLabel })}</h2>
          <label className="toolbar-check">
            <input
              type="checkbox"
              checked={hideNoise}
              onChange={(e) => setHideNoise(e.target.checked)}
            />
            {t("threads.hideNoise")}
            {noiseHidden > 0 ? ` (${noiseHidden})` : ""}
          </label>
          <label>
            {t("threads.state")}{" "}
            <select
              value={stateFilter}
              onChange={(e) => setStateFilter(e.target.value as StateFilter)}
            >
              <option value="ALL">{t("states.all")}</option>
              {analysis.state_counts.map((s) => (
                <option key={s.state} value={s.state}>
                  {s.state}
                </option>
              ))}
            </select>
          </label>
          <span className="threads-sort-hint">{t("threads.sortHint")}</span>
        </div>
        {filteredThreads.length === 0 ? (
          <p className="empty">{t("threads.empty")}</p>
        ) : (
          <div className="table-scroll">
            <table
              className="threads-table"
              data-testid="threads-table"
              data-has-waiting={showWaitingOn ? "true" : "false"}
            >
            <thead>
              <tr>
                <SortHeader
                  label={t("threads.colName")}
                  column="name"
                  sorts={activeSorts}
                  hint={t("threads.sortHint")}
                  testId="sort-name"
                  onSort={onSort}
                />
                <SortHeader
                  label={t("threads.colId")}
                  column="id"
                  sorts={activeSorts}
                  hint={t("threads.sortHint")}
                  testId="sort-id"
                  onSort={onSort}
                />
                <SortHeader
                  label={t("threads.colState")}
                  column="state"
                  sorts={activeSorts}
                  hint={t("threads.sortHint")}
                  testId="sort-state"
                  onSort={onSort}
                />
                {showWaitingOn ? (
                  <SortHeader
                    label={t("threads.colWaitingOn")}
                    column="waiting"
                    sorts={activeSorts}
                    hint={t("threads.sortHint")}
                    testId="sort-waiting"
                    onSort={onSort}
                  />
                ) : null}
                <SortHeader
                  label={t("threads.colStack")}
                  column="stack"
                  sorts={activeSorts}
                  hint={t("threads.sortHint")}
                  onSort={onSort}
                />
                <SortHeader
                  label={t("threads.colHeldLocks")}
                  column="locks"
                  sorts={activeSorts}
                  hint={t("threads.sortHint")}
                  onSort={onSort}
                />
              </tr>
            </thead>
            <tbody>
              {filteredThreads.map(({ t: th, index }) => (
                <ThreadRow
                  key={threadDomId(index)}
                  thread={th}
                  index={index}
                  focused={focusIndex === index}
                  expanded={expandedStacks.has(index)}
                  showWaitingOn={showWaitingOn}
                  onToggleStack={() => toggleStack(index)}
                />
              ))}
            </tbody>
          </table>
          </div>
        )}
      </section>

      <section
        className="panel flame-panel"
        data-testid="flamegraph"
        id={sectionDomId("flamegraph")}
      >
        <h2>{t("flame.title")}</h2>
        <p className="empty">{t("flame.blurb")}</p>
        <FlameGraph threads={flameThreads} />
      </section>
    </main>
    </div>
  );
}

function colClass(column: ThreadSortKey): string {
  switch (column) {
    case "name":
      return "col-name";
    case "id":
      return "col-id";
    case "state":
      return "col-state";
    case "waiting":
      return "col-waiting";
    case "stack":
      return "col-stack";
    case "locks":
      return "col-locks";
  }
}

function SortHeader({
  label,
  column,
  sorts,
  hint,
  testId,
  onSort,
}: {
  label: string;
  column: ThreadSortKey;
  sorts: ThreadSortSpec[];
  hint: string;
  testId?: string;
  onSort: (key: ThreadSortKey, additive: boolean) => void;
}) {
  const rank = sorts.findIndex((s) => s.key === column);
  const spec = rank >= 0 ? sorts[rank] : undefined;
  const ariaSort =
    rank !== 0 || !spec
      ? "none"
      : spec.dir === "asc"
        ? "ascending"
        : "descending";
  return (
    <th className={colClass(column)} aria-sort={ariaSort}>
      <button
        type="button"
        className={`th-btn${spec ? " is-active" : ""}`}
        data-testid={testId}
        data-sort-rank={rank >= 0 ? String(rank + 1) : undefined}
        data-sort-dir={spec?.dir}
        title={hint}
        onClick={(e) => onSort(column, e.shiftKey)}
      >
        {label}
        {spec ? (
          <span className="th-sort-mark" aria-hidden="true">
            {spec.dir === "asc" ? "↑" : "↓"}
            {sorts.length > 1 ? rank + 1 : ""}
          </span>
        ) : null}
      </button>
    </th>
  );
}

function StackFrameView({ frame }: { frame: string }) {
  return (
    <>
      {tokenizeStackFrame(frame).map((tok, i) => (
        <span key={i} className={`sf sf-${tok.kind}`}>
          {tok.text}
        </span>
      ))}
    </>
  );
}

function ThreadRow({
  thread: th,
  index,
  focused,
  expanded,
  showWaitingOn,
  onToggleStack,
}: {
  thread: ThreadInfo;
  index: number;
  focused: boolean;
  expanded: boolean;
  showWaitingOn: boolean;
  onToggleStack: () => void;
}) {
  return (
    <>
      <tr
        id={threadDomId(index)}
        className={focused ? "thread-row focus" : "thread-row"}
      >
        <td className="cell-break col-name" data-testid="thread-name">{th.name}</td>
        <td className="col-id" data-testid="thread-id">{th.id ?? ""}</td>
        <td className="col-state" data-testid="thread-state">
          <span
            className="state-pill"
            style={{ background: STATE_COLORS[th.state] ?? "#64748b" }}
          >
            {th.state}
          </span>
        </td>
        {showWaitingOn ? (
          <td className="mono cell-break col-waiting">{th.waiting_on ?? ""}</td>
        ) : null}
        <td className="col-stack">
          {th.stack_depth > 0 ? (
            <button type="button" className="linkish" onClick={onToggleStack}>
              {th.stack_depth}
              {expanded ? " ▾" : " ▸"}
            </button>
          ) : (
            0
          )}
        </td>
        <td className="held-locks-cell col-locks">
          {th.held_locks.length === 0 ? (
            ""
          ) : (
            <ul className="held-locks-list">
              {th.held_locks.map((lock, i) => (
                <li key={`${lock}-${i}`} className="mono cell-break">
                  {lock}
                </li>
              ))}
            </ul>
          )}
        </td>
      </tr>
      {expanded && th.stack.length > 0 && (
        <tr className="stack-row">
          <td colSpan={showWaitingOn ? 6 : 5}>
            <ol className="stack-preview" data-testid="stack-preview">
              {th.stack.map((f, i) => (
                <li key={i} className="mono" data-testid="stack-frame">
                  <StackFrameView frame={f} />
                </li>
              ))}
            </ol>
          </td>
        </tr>
      )}
    </>
  );
}
