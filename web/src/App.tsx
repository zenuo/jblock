import { useCallback, useEffect, useRef, useState } from "react";
import {
  analyze,
  generateJava,
  classNameFor,
  isWasmReady,
  preloadWasm,
  type JavaScenario,
} from "./analyzer";
import { exportHtml, exportPdf } from "./export";
import { useI18n } from "./i18n";
import LanguageMenu from "./LanguageMenu";
import Results from "./Results";
import { SAMPLE_DUMP } from "./sampleDump";
import type { Analysis } from "./types";

type BusyPhase = "wasm" | "analyzing";

export default function App() {
  const { t, locale } = useI18n();
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [sourceName, setSourceName] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [busyPhase, setBusyPhase] = useState<BusyPhase | null>(null);
  const [wasmReady, setWasmReady] = useState(() => isWasmReady());
  const fileInputRef = useRef<HTMLInputElement>(null);
  const busy = busyPhase !== null;

  const [codegenOpen, setCodegenOpen] = useState(false);
  const [javaScenario, setJavaScenario] = useState<JavaScenario>("deadlock");
  const [javaCount, setJavaCount] = useState(3);
  const [javaCode, setJavaCode] = useState<string>("");
  const [codegenError, setCodegenError] = useState<string | null>(null);
  const closeCodegenBtnRef = useRef<HTMLButtonElement>(null);

  // Background-load WASM as soon as the page mounts.
  useEffect(() => {
    let cancelled = false;
    void preloadWasm()
      .then(() => {
        if (!cancelled) setWasmReady(true);
      })
      .catch((e) => {
        if (!cancelled) {
          const msg = e instanceof Error ? e.message : String(e);
          setError(t("app.wasmLoadFailed", { error: msg }));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

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

  const runAnalysis = useCallback(
    async (text: string, name: string) => {
      setError(null);
      setBusyPhase(isWasmReady() || wasmReady ? "analyzing" : "wasm");
      try {
        if (!isWasmReady()) {
          setBusyPhase("wasm");
          await preloadWasm();
          setWasmReady(true);
        }
        setBusyPhase("analyzing");
        await new Promise<void>((resolve) => {
          window.setTimeout(resolve, 0);
        });
        const result = await analyze(text);
        setAnalysis(result);
        setSourceName(name);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setAnalysis(null);
      } finally {
        setBusyPhase(null);
      }
    },
    [wasmReady],
  );

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

  return (
    <div
      className={`app${dragging ? " dragging" : ""}`}
      onDrop={onDrop}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
    >
      {dragging && <div className="drop-overlay">{t("app.dropOverlay")}</div>}
      {busyPhase && (
        <div
          className="loading-overlay"
          role="status"
          aria-live="polite"
          data-testid="loading-overlay"
        >
          <div className="loading-card">
            <div className="spinner" aria-hidden="true" />
            <p>
              {busyPhase === "wasm" ? t("app.loadingWasm") : t("app.analyzing")}
            </p>
          </div>
        </div>
      )}
      <header className="app-header">
        <div className="app-header-row">
          <h1>
            <span className="logo">jblock</span> {t("app.title")}
          </h1>
          <div className="header-actions">
            <LanguageMenu />
            <button
              type="button"
              className="btn"
              data-testid="open-codegen"
              onClick={openCodegen}
            >
              {t("app.generateJava")}
            </button>
          </div>
        </div>
        <p className="tagline">{t("app.tagline")}</p>
      </header>

      <section className="controls">
        <label className={`btn primary${busy ? " disabled" : ""}`}>
          {t("app.chooseDump")}
          <input
            ref={fileInputRef}
            type="file"
            accept=".txt,.log,.tdump,.dump,text/plain"
            hidden
            disabled={busy}
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
          {t("app.loadSample")}
        </button>
        {analysis && (
          <>
            <button
              className="btn"
              onClick={() => exportHtml(analysis, sourceName, t, locale)}
              disabled={busy}
            >
              {t("app.exportHtml")}
            </button>
            <button
              className="btn"
              onClick={() => void exportPdf(analysis, sourceName)}
              disabled={busy}
            >
              {t("app.exportPdf")}
            </button>
          </>
        )}
      </section>

      {error && (
        <p className="status error">
          {t("app.errorPrefix")} {error}
        </p>
      )}

      {analysis && !busy && <Results analysis={analysis} />}

      {!analysis && !busy && <p className="hint">{t("app.hint")}</p>}

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
              <h2 id="codegen-title">{t("codegen.title")}</h2>
              <button
                ref={closeCodegenBtnRef}
                type="button"
                className="modal-close"
                aria-label={t("codegen.close")}
                onClick={closeCodegen}
              >
                ×
              </button>
            </div>
            <p className="empty">{t("codegen.blurb")}</p>
            <div className="codegen-controls">
              <label>
                {t("codegen.scenario")}{" "}
                <select
                  value={javaScenario}
                  onChange={(e) =>
                    setJavaScenario(e.target.value as JavaScenario)
                  }
                >
                  <option value="deadlock">{t("codegen.deadlock")}</option>
                  <option value="lock-contention">
                    {t("codegen.lockContention")}
                  </option>
                  <option value="thread-pool-exhaustion">
                    {t("codegen.threadPoolExhaustion")}
                  </option>
                  <option value="sync-io-hotspot">
                    {t("codegen.syncIoHotspot")}
                  </option>
                  <option value="dangerous-hot-lock">
                    {t("codegen.dangerousHotLock")}
                  </option>
                  <option value="connection-pool-starve">
                    {t("codegen.connectionPoolStarve")}
                  </option>
                  <option value="future-latch-deadlock">
                    {t("codegen.futureLatchDeadlock")}
                  </option>
                  <option value="logging-appender-contention">
                    {t("codegen.loggingAppenderContention")}
                  </option>
                  <option value="busy-wait-spin">
                    {t("codegen.busyWaitSpin")}
                  </option>
                  <option value="condition-starvation">
                    {t("codegen.conditionStarvation")}
                  </option>
                  <option value="lock-order-risk">
                    {t("codegen.lockOrderRisk")}
                  </option>
                </select>
              </label>
              <label>
                {t("codegen.threads")}{" "}
                <input
                  type="number"
                  min={2}
                  max={64}
                  value={javaCount}
                  onChange={(e) => setJavaCount(Number(e.target.value))}
                />
              </label>
              <button
                type="button"
                className="btn primary"
                onClick={onGenerateJava}
              >
                {t("codegen.generate")}
              </button>
              {javaCode && (
                <button type="button" className="btn" onClick={downloadJava}>
                  {t("codegen.download")}
                </button>
              )}
            </div>
            {codegenError && (
              <p className="status error">
                {t("app.errorPrefix")} {codegenError}
              </p>
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
