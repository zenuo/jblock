import { useEffect, useMemo, useState } from "react";
import {
  aggregateContention,
  buildFindings,
  clusterByStack,
  isJvmNoise,
  sortThreads,
  threadDomId,
  type Finding,
  type ThreadSortKey,
} from "./analysisUi";
import { useI18n, type TranslateFn } from "./i18n";
import PatternLegendModal from "./PatternLegendModal";
import type { Analysis, ThreadInfo } from "./types";

const STATE_COLORS: Record<string, string> = {
  RUNNABLE: "#22c55e",
  BLOCKED: "#ef4444",
  WAITING: "#f59e0b",
  TIMED_WAITING: "#eab308",
  NEW: "#38bdf8",
  TERMINATED: "#94a3b8",
};

type StateFilter = "ALL" | string;

/** Initial frames shown when a stack row is expanded; remainder via "show all". */
const STACK_PREVIEW_FRAMES = 12;

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
  const [sortKey, setSortKey] = useState<ThreadSortKey>("name");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");
  const [expandedLocks, setExpandedLocks] = useState<Set<string>>(new Set());
  const [expandedStacks, setExpandedStacks] = useState<Set<number>>(new Set());
  const [fullStacks, setFullStacks] = useState<Set<number>>(new Set());
  const [focusIndex, setFocusIndex] = useState<number | null>(null);
  const [legendFinding, setLegendFinding] = useState<Finding | null>(null);

  useEffect(() => {
    setStateFilter(hasBlocked ? "BLOCKED" : "ALL");
    setExpandedLocks(new Set());
    setExpandedStacks(new Set());
    setFullStacks(new Set());
    setFocusIndex(null);
  }, [analysis, hasBlocked]);

  const filteredThreads = useMemo(() => {
    let list = analysis.threads.map((th, index) => ({ t: th, index }));
    if (hideNoise) list = list.filter(({ t: th }) => !isJvmNoise(th.name));
    if (stateFilter !== "ALL") {
      list = list.filter(({ t: th }) => th.state === stateFilter);
    }
    const sorted = sortThreads(
      list.map(({ t: th }) => th),
      sortKey,
      sortDir,
    );
    return sorted.map((th) => {
      const index = analysis.threads.indexOf(th);
      return { t: th, index };
    });
  }, [analysis.threads, hideNoise, stateFilter, sortKey, sortDir]);

  const clusters = useMemo(() => {
    const base = hideNoise
      ? analysis.threads.filter((th) => !isJvmNoise(th.name))
      : analysis.threads;
    const scoped =
      stateFilter === "ALL" ? base : base.filter((th) => th.state === stateFilter);
    return clusterByStack(scoped, 2).slice(0, 12);
  }, [analysis.threads, hideNoise, stateFilter]);

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
      if (next.has(index)) {
        next.delete(index);
        setFullStacks((full) => {
          if (!full.has(index)) return full;
          const cleared = new Set(full);
          cleared.delete(index);
          return cleared;
        });
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const showFullStack = (index: number) => {
    setFullStacks((prev) => {
      if (prev.has(index)) return prev;
      const next = new Set(prev);
      next.add(index);
      return next;
    });
  };

  const onSort = (key: ThreadSortKey) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const shownLabel =
    filteredThreads.length !== analysis.threads.length
      ? `${filteredThreads.length} / ${analysis.threads.length}`
      : String(filteredThreads.length);

  return (
    <main className="results" data-testid="results">
      <section className="panel findings" data-testid="findings">
        <div className="findings-header">
          <h2>{t("findings.title")}</h2>
          <div className="findings-header-meta">
            <span className="meta mono">
              {t("findings.meta", {
                count: analysis.total_threads,
                format: analysis.format,
              })}
            </span>
            {analysis.java_version && (
              <span
                className="java-version-badge"
                data-testid="findings-java-version"
              >
                {t("findings.javaVersion", { version: analysis.java_version })}
              </span>
            )}
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
              {analysis.java_version
                ? t("findings.okDetailWithJava", {
                    version: analysis.java_version,
                    count: analysis.total_threads,
                    format: analysis.format,
                  })
                : t("findings.okDetail", {
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
        <section className="panel deadlock-panel" data-testid="deadlocks">
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

      <section className="panel" data-testid="contention">
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

      <section className="panel states-panel" data-testid="thread-states">
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
        <section className="panel" data-testid="clusters">
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
                      {f}
                    </li>
                  ))}
                </ol>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className="panel" data-testid="threads">
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
        </div>
        {filteredThreads.length === 0 ? (
          <p className="empty">{t("threads.empty")}</p>
        ) : (
          <div className="table-scroll">
            <table className="threads-table">
            <thead>
              <tr>
                <th>
                  <button
                    type="button"
                    className="th-btn"
                    onClick={() => onSort("name")}
                  >
                    {t("threads.colName")}
                  </button>
                </th>
                <th>{t("threads.colId")}</th>
                <th>
                  <button
                    type="button"
                    className="th-btn"
                    onClick={() => onSort("state")}
                  >
                    {t("threads.colState")}
                  </button>
                </th>
                <th>{t("threads.colWaitingOn")}</th>
                <th>
                  <button
                    type="button"
                    className="th-btn"
                    onClick={() => onSort("stack")}
                  >
                    {t("threads.colStack")}
                  </button>
                </th>
                <th>
                  <button
                    type="button"
                    className="th-btn"
                    onClick={() => onSort("locks")}
                  >
                    {t("threads.colHeldLocks")}
                  </button>
                </th>
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
                  showFullStack={fullStacks.has(index)}
                  onToggleStack={() => toggleStack(index)}
                  onShowFullStack={() => showFullStack(index)}
                  translate={t}
                />
              ))}
            </tbody>
          </table>
          </div>
        )}
      </section>
    </main>
  );
}

function ThreadRow({
  thread: th,
  index,
  focused,
  expanded,
  showFullStack,
  onToggleStack,
  onShowFullStack,
  translate: tr,
}: {
  thread: ThreadInfo;
  index: number;
  focused: boolean;
  expanded: boolean;
  showFullStack: boolean;
  onToggleStack: () => void;
  onShowFullStack: () => void;
  translate: TranslateFn;
}) {
  const previewCap = STACK_PREVIEW_FRAMES;
  const truncated = !showFullStack && th.stack.length > previewCap;
  const visibleFrames = truncated ? th.stack.slice(0, previewCap) : th.stack;
  const hiddenCount = truncated ? th.stack.length - previewCap : 0;

  return (
    <>
      <tr
        id={threadDomId(index)}
        className={focused ? "thread-row focus" : "thread-row"}
      >
        <td className="cell-break">{th.name}</td>
        <td>{th.id ?? ""}</td>
        <td>
          <span
            className="state-pill"
            style={{ background: STATE_COLORS[th.state] ?? "#64748b" }}
          >
            {th.state}
          </span>
        </td>
        <td className="mono cell-break">{th.waiting_on ?? ""}</td>
        <td>
          {th.stack_depth > 0 ? (
            <button type="button" className="linkish" onClick={onToggleStack}>
              {th.stack_depth}
              {expanded ? " ▾" : " ▸"}
            </button>
          ) : (
            0
          )}
        </td>
        <td className="held-locks-cell">
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
          <td colSpan={6}>
            <ol className="stack-preview" data-testid="stack-preview">
              {visibleFrames.map((f, i) => (
                <li key={i} className="mono">
                  {f}
                </li>
              ))}
              {hiddenCount > 0 && (
                <li className="stack-more">
                  <button
                    type="button"
                    className="linkish stack-more-btn"
                    data-testid="stack-show-all"
                    onClick={onShowFullStack}
                  >
                    {tr("threads.moreFrames", { count: hiddenCount })}
                  </button>
                </li>
              )}
            </ol>
          </td>
        </tr>
      )}
    </>
  );
}
