import { useCallback, useRef, useState } from "react";
import { analyze } from "./analyzer";
import { exportHtml, exportPdf } from "./export";
import type { Analysis } from "./types";

const SAMPLE_DUMP = `"main" #1 prio=5 os_prio=0 tid=0x00007f0001 nid=0x1 waiting for monitor entry [0x00007f0002]
   java.lang.Thread.State: BLOCKED (on object monitor)
        at com.example.App.run(App.java:10)
        - waiting to lock <0x000000076ab00000> (a java.lang.Object)
        - locked <0x000000076ab11111> (a java.lang.Object)

"worker" #2 prio=5 os_prio=0 tid=0x00007f0003 nid=0x2 runnable [0x00007f0004]
   java.lang.Thread.State: RUNNABLE
        at com.example.Worker.work(Worker.java:20)
        - locked <0x000000076ab00000> (a java.lang.Object)

"scheduler" #3 prio=5 os_prio=0 tid=0x00007f0005 nid=0x3 waiting on condition [0x00007f0006]
   java.lang.Thread.State: TIMED_WAITING (sleeping)
        at java.lang.Thread.sleep(Native Method)
`;

const STATE_COLORS: Record<string, string> = {
  RUNNABLE: "#22c55e",
  BLOCKED: "#ef4444",
  WAITING: "#f59e0b",
  TIMED_WAITING: "#eab308",
  NEW: "#38bdf8",
  TERMINATED: "#94a3b8",
};

export default function App() {
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [sourceName, setSourceName] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const runAnalysis = useCallback(async (text: string, name: string) => {
    setBusy(true);
    setError(null);
    try {
      const result = await analyze(text);
      setAnalysis(result);
      setSourceName(name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setAnalysis(null);
    } finally {
      setBusy(false);
    }
  }, []);

  const onFile = useCallback(
    async (file: File) => {
      const text = await file.text();
      await runAnalysis(text, file.name);
    },
    [runAnalysis],
  );

  const maxState = analysis
    ? Math.max(1, ...analysis.state_counts.map((s) => s.count))
    : 1;

  return (
    <div className="app">
      <header className="app-header">
        <h1>
          <span className="logo">jblock</span> Java Thread Dump Analyzer
        </h1>
        <p className="tagline">
          Parse jstack / ThreadMXBean dumps locally in your browser via Rust + WebAssembly.
        </p>
      </header>

      <section className="controls">
        <label className="btn primary">
          Choose thread dump…
          <input
            ref={fileInputRef}
            type="file"
            accept=".txt,.log,.tdump,.dump,text/plain"
            hidden
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void onFile(f);
            }}
          />
        </label>
        <button
          className="btn"
          onClick={() => void runAnalysis(SAMPLE_DUMP, "sample.txt")}
          disabled={busy}
        >
          Load sample
        </button>
        {analysis && (
          <>
            <button className="btn" onClick={() => exportHtml(analysis, sourceName)}>
              Export HTML
            </button>
            <button className="btn" onClick={() => exportPdf(analysis, sourceName)}>
              Export PDF
            </button>
          </>
        )}
      </section>

      {busy && <p className="status">Analyzing…</p>}
      {error && <p className="status error">Error: {error}</p>}

      {analysis && (
        <main className="results" data-testid="results">
          <div className="summary">
            <div className="stat">
              <span className="stat-value">{analysis.total_threads}</span>
              <span className="stat-label">threads</span>
            </div>
            <div className="stat">
              <span className="stat-value">{analysis.format}</span>
              <span className="stat-label">format</span>
            </div>
            <div className="stat">
              <span className="stat-value">{analysis.blocked_edges.length}</span>
              <span className="stat-label">lock contentions</span>
            </div>
          </div>

          <section className="panel">
            <h2>Thread states</h2>
            <ul className="states">
              {analysis.state_counts.map((s) => (
                <li key={s.state}>
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
                </li>
              ))}
            </ul>
          </section>

          <section className="panel">
            <h2>Lock contention</h2>
            {analysis.blocked_edges.length === 0 ? (
              <p className="empty">No blocked threads detected.</p>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th>Blocked thread</th>
                    <th>Lock</th>
                    <th>Held by</th>
                  </tr>
                </thead>
                <tbody>
                  {analysis.blocked_edges.map((e, i) => (
                    <tr key={i}>
                      <td>{e.blocked_thread}</td>
                      <td className="mono">{e.lock}</td>
                      <td>{e.owner_thread ?? "(unknown)"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          <section className="panel">
            <h2>Threads ({analysis.threads.length})</h2>
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Id</th>
                  <th>State</th>
                  <th>Stack</th>
                  <th>Held locks</th>
                </tr>
              </thead>
              <tbody>
                {analysis.threads.map((t, i) => (
                  <tr key={i}>
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
                    <td>{t.stack_depth}</td>
                    <td className="mono">{t.held_locks.join(", ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </main>
      )}

      {!analysis && !busy && (
        <p className="hint">
          Select a thread dump file or load the sample to get started.
        </p>
      )}
    </div>
  );
}
