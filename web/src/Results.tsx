import { useEffect, useMemo, useState } from "react";
import {
  aggregateContention,
  buildFindings,
  clusterByStack,
  isJvmNoise,
  sortThreads,
  threadDomId,
  type ThreadSortKey,
} from "./analysisUi";
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

interface Props {
  analysis: Analysis;
}

export default function Results({ analysis }: Props) {
  const findings = useMemo(() => buildFindings(analysis), [analysis]);
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
  const [focusIndex, setFocusIndex] = useState<number | null>(null);

  useEffect(() => {
    setStateFilter(hasBlocked ? "BLOCKED" : "ALL");
    setExpandedLocks(new Set());
    setExpandedStacks(new Set());
    setFocusIndex(null);
  }, [analysis, hasBlocked]);

  const filteredThreads = useMemo(() => {
    let list = analysis.threads.map((t, index) => ({ t, index }));
    if (hideNoise) list = list.filter(({ t }) => !isJvmNoise(t.name));
    if (stateFilter !== "ALL") {
      list = list.filter(({ t }) => t.state === stateFilter);
    }
    const sorted = sortThreads(
      list.map(({ t }) => t),
      sortKey,
      sortDir,
    );
    // Re-attach original indices after sort (match by object identity).
    return sorted.map((t) => {
      const index = analysis.threads.indexOf(t);
      return { t, index };
    });
  }, [analysis.threads, hideNoise, stateFilter, sortKey, sortDir]);

  const clusters = useMemo(() => {
    const base = hideNoise
      ? analysis.threads.filter((t) => !isJvmNoise(t.name))
      : analysis.threads;
    const scoped =
      stateFilter === "ALL" ? base : base.filter((t) => t.state === stateFilter);
    return clusterByStack(scoped, 2).slice(0, 12);
  }, [analysis.threads, hideNoise, stateFilter]);

  const maxState = Math.max(1, ...analysis.state_counts.map((s) => s.count));
  const noiseHidden = hideNoise
    ? analysis.threads.filter((t) => isJvmNoise(t.name)).length
    : 0;

  useEffect(() => {
    if (focusIndex === null) return;
    const el = document.getElementById(threadDomId(focusIndex));
    el?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [focusIndex, filteredThreads]);

  const focusThreadByName = (name: string) => {
    const idx = analysis.threads.findIndex((t) => t.name === name);
    if (idx < 0) return;
    const t = analysis.threads[idx];
    if (hideNoise && isJvmNoise(name)) setHideNoise(false);
    if (stateFilter !== "ALL" && t.state !== stateFilter) setStateFilter("ALL");
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

  const onSort = (key: ThreadSortKey) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  return (
    <main className="results" data-testid="results">
      <section className="panel findings" data-testid="findings">
        <div className="findings-header">
          <h2>Findings</h2>
          <span className="meta mono">
            {analysis.total_threads} threads · {analysis.format}
          </span>
        </div>
        <ul className="findings-list">
          {findings.map((f, i) => (
            <li key={i} className={`finding finding-${f.severity}`}>
              <strong>{f.title}</strong>
              <span className="mono">{f.detail}</span>
            </li>
          ))}
        </ul>
      </section>

      {analysis.deadlocks.length > 0 && (
        <section className="panel deadlock-panel" data-testid="deadlocks">
          <h2>Deadlocks detected ({analysis.deadlocks.length})</h2>
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
                    waits on {e.lock} (held by{" "}
                    {e.owner_thread ? (
                      <button
                        type="button"
                        className="linkish"
                        onClick={() => focusThreadByName(e.owner_thread!)}
                      >
                        {e.owner_thread}
                      </button>
                    ) : (
                      "unknown"
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
        <h2>Lock contention ({contentionGroups.length} lock(s))</h2>
        {contentionGroups.length === 0 ? (
          <p className="empty">No blocked threads detected.</p>
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
                      held by{" "}
                      {g.owner_thread ? (
                        <button
                          type="button"
                          className="linkish"
                          onClick={() => focusThreadByName(g.owner_thread!)}
                        >
                          {g.owner_thread}
                        </button>
                      ) : (
                        "(unknown)"
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

      <section className="panel">
        <h2>Thread states</h2>
        <ul className="states">
          <li>
            <button
              type="button"
              className={`state-filter${stateFilter === "ALL" ? " active" : ""}`}
              onClick={() => setStateFilter("ALL")}
            >
              <span className="state-name">ALL</span>
              <span className="bar-track">
                <span
                  className="bar-fill"
                  style={{ width: "100%", background: "#94a3b8" }}
                />
              </span>
              <span className="state-count">{analysis.total_threads}</span>
            </button>
          </li>
          {analysis.state_counts.map((s) => (
            <li key={s.state}>
              <button
                type="button"
                className={`state-filter${stateFilter === s.state ? " active" : ""}`}
                onClick={() => setStateFilter(s.state)}
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
          <h2>Stack clusters ({clusters.length})</h2>
          <p className="empty">
            Threads sharing the same top frames (duplicates collapsed).
          </p>
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
          <h2>
            Threads ({filteredThreads.length}
            {filteredThreads.length !== analysis.threads.length
              ? ` / ${analysis.threads.length}`
              : ""}
            )
          </h2>
          <label className="toolbar-check">
            <input
              type="checkbox"
              checked={hideNoise}
              onChange={(e) => setHideNoise(e.target.checked)}
            />
            Hide JVM noise
            {noiseHidden > 0 ? ` (${noiseHidden})` : ""}
          </label>
          <label>
            State{" "}
            <select
              value={stateFilter}
              onChange={(e) => setStateFilter(e.target.value as StateFilter)}
            >
              <option value="ALL">ALL</option>
              {analysis.state_counts.map((s) => (
                <option key={s.state} value={s.state}>
                  {s.state}
                </option>
              ))}
            </select>
          </label>
        </div>
        {filteredThreads.length === 0 ? (
          <p className="empty">No threads match the current filters.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>
                  <button type="button" className="th-btn" onClick={() => onSort("name")}>
                    Name
                  </button>
                </th>
                <th>Id</th>
                <th>
                  <button type="button" className="th-btn" onClick={() => onSort("state")}>
                    State
                  </button>
                </th>
                <th>Waiting on</th>
                <th>
                  <button type="button" className="th-btn" onClick={() => onSort("stack")}>
                    Stack
                  </button>
                </th>
                <th>
                  <button type="button" className="th-btn" onClick={() => onSort("locks")}>
                    Held locks
                  </button>
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredThreads.map(({ t, index }) => (
                <ThreadRow
                  key={threadDomId(index)}
                  thread={t}
                  index={index}
                  focused={focusIndex === index}
                  expanded={expandedStacks.has(index)}
                  onToggleStack={() => toggleStack(index)}
                />
              ))}
            </tbody>
          </table>
        )}
      </section>
    </main>
  );
}

function ThreadRow({
  thread: t,
  index,
  focused,
  expanded,
  onToggleStack,
}: {
  thread: ThreadInfo;
  index: number;
  focused: boolean;
  expanded: boolean;
  onToggleStack: () => void;
}) {
  return (
    <>
      <tr
        id={threadDomId(index)}
        className={focused ? "thread-row focus" : "thread-row"}
      >
        <td>{t.name}</td>
        <td>{t.id ?? ""}</td>
        <td>
          <span
            className="state-pill"
            style={{ background: STATE_COLORS[t.state] ?? "#64748b" }}
          >
            {t.state}
          </span>
        </td>
        <td className="mono">{t.waiting_on ?? ""}</td>
        <td>
          {t.stack_depth > 0 ? (
            <button type="button" className="linkish" onClick={onToggleStack}>
              {t.stack_depth}
              {expanded ? " ▾" : " ▸"}
            </button>
          ) : (
            0
          )}
        </td>
        <td className="mono">{t.held_locks.join(", ")}</td>
      </tr>
      {expanded && t.stack.length > 0 && (
        <tr className="stack-row">
          <td colSpan={6}>
            <ol className="stack-preview">
              {t.stack.map((f, i) => (
                <li key={i} className="mono">
                  {f}
                </li>
              ))}
              {t.stack_depth > t.stack.length && (
                <li className="empty">
                  … {t.stack_depth - t.stack.length} more frame(s)
                </li>
              )}
            </ol>
          </td>
        </tr>
      )}
    </>
  );
}
