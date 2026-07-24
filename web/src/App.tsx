import { useCallback, useEffect, useRef, useState } from "react";
import { analyze, generateJava, classNameFor, type JavaScenario } from "./analyzer";
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

  const [codegenOpen, setCodegenOpen] = useState(false);
  const [javaScenario, setJavaScenario] = useState<JavaScenario>("deadlock");
  const [javaCount, setJavaCount] = useState(3);
  const [javaCode, setJavaCode] = useState<string>("");
  const [codegenError, setCodegenError] = useState<string | null>(null);
  const closeCodegenBtnRef = useRef<HTMLButtonElement>(null);

  const closeCodegen = useCallback(() => {
    setCodegenOpen(false);
    setCodegenError(null);
  }, []);

  const openCodegen = useCallback(() => {
    setCodegenOpen(true);
    setCodegenError(null);
  }, []);

  useEffect(() => {
    if (!codegenOpen) return;
    closeCodegenBtnRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeCodegen();
    };
    window.addEventListener("keydown", onKey);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = prevOverflow;
    };
  }, [codegenOpen, closeCodegen]);

  const onGenerateJava = useCallback(() => {
    setCodegenError(null);
    try {
      setJavaCode(generateJava(javaScenario, javaCount));
    } catch (e) {
      setCodegenError(e instanceof Error ? e.message : String(e));
    }
  }, [javaScenario, javaCount]);

  const downloadJava = useCallback(() => {
    const className = classNameFor(javaScenario);
    const blob = new Blob([javaCode], { type: "text/x-java-source" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${className}.java`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [javaCode, javaScenario]);

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

  const [dragging, setDragging] = useState(false);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      const file = e.dataTransfer.files?.[0];
      if (file) void onFile(file);
    },
    [onFile],
  );

  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragging(true);
  }, []);

  const onDragLeave = useCallback((e: React.DragEvent) => {
    if (e.currentTarget === e.target) setDragging(false);
  }, []);

  const maxState = analysis
    ? Math.max(1, ...analysis.state_counts.map((s) => s.count))
    : 1;

  return (
    <div
      className={`app${dragging ? " dragging" : ""}`}
      onDrop={onDrop}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
      {dragging && <div className="drop-overlay">Drop thread dump to analyze</div>}
      <header className="app-header">
        <div className="app-header-row">
          <h1>
            <span className="logo">jblock</span> Java Thread Dump Analyzer
          </h1>
          <button
            type="button"
            className="btn"
            data-testid="open-codegen"
            onClick={openCodegen}
          >
            Generate Java…
          </button>
        </div>
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
            <button className="btn" onClick={() => void exportPdf(analysis, sourceName)}>
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
            <div className={`stat${analysis.deadlocks.length > 0 ? " danger" : ""}`}>
              <span className="stat-value">{analysis.deadlocks.length}</span>
              <span className="stat-label">deadlocks</span>
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

          {analysis.deadlocks.length > 0 && (
            <section className="panel deadlock-panel" data-testid="deadlocks">
              <h2>⚠ Deadlocks detected ({analysis.deadlocks.length})</h2>
              {analysis.deadlocks.map((d, i) => (
                <div key={i} className="deadlock-cycle">
                  <span className="mono">
                    {d.threads.join(" → ")} → {d.threads[0]}
                  </span>
                  <ul>
                    {d.edges.map((e, j) => (
                      <li key={j} className="mono">
                        {e.blocked_thread} waits on {e.lock} (held by{" "}
                        {e.owner_thread ?? "unknown"})
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </section>
          )}

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

      {codegenOpen && (
        <div
          className="modal-backdrop"
          data-testid="codegen-modal"
          onClick={(e) => {
            if (e.target === e.currentTarget) closeCodegen();
          }}
        >
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="codegen-title"
          >
            <div className="modal-header">
              <h2 id="codegen-title">Generate Java reproducer</h2>
              <button
                ref={closeCodegenBtnRef}
                type="button"
                className="modal-close"
                aria-label="Close"
                onClick={closeCodegen}
              >
                ×
              </button>
            </div>
            <p className="empty">
              Emit a runnable Java program that reproduces a thread problem, then
              capture its dump with <span className="mono">jstack</span> and analyze
              it on the main page.
            </p>
            <div className="codegen-controls">
              <label>
                Scenario{" "}
                <select
                  value={javaScenario}
                  onChange={(e) => setJavaScenario(e.target.value as JavaScenario)}
                >
                  <option value="deadlock">Deadlock cycle</option>
                  <option value="lock-contention">Lock contention</option>
                </select>
              </label>
              <label>
                Threads{" "}
                <input
                  type="number"
                  min={2}
                  max={64}
                  value={javaCount}
                  onChange={(e) => setJavaCount(Number(e.target.value))}
                />
              </label>
              <button type="button" className="btn primary" onClick={onGenerateJava}>
                Generate
              </button>
              {javaCode && (
                <button type="button" className="btn" onClick={downloadJava}>
                  Download .java
                </button>
              )}
            </div>
            {codegenError && (
              <p className="status error">Error: {codegenError}</p>
            )}
            {javaCode && (
              <pre className="code-block" data-testid="java-code">
                <code>{javaCode}</code>
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
