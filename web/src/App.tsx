import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  analyzeMany,
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
import type { Analysis, MultiDumpAnalysis, PatternHit } from "./types";

type BusyPhase = "wasm" | "analyzing";

function mergeCrossPatterns(dump: Analysis, cross: PatternHit[]): Analysis {
  if (cross.length === 0) return dump;
  return { ...dump, patterns: [...cross, ...dump.patterns] };
}

export default function App() {
  const { t, locale } = useI18n();
  const [series, setSeries] = useState<MultiDumpAnalysis | null>(null);
  const [dumpNames, setDumpNames] = useState<string[]>([]);
  const [selectedDump, setSelectedDump] = useState(0);
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

  const analysis = useMemo(() => {
    if (!series || series.dumps.length === 0) return null;
    const idx = Math.min(selectedDump, series.dumps.length - 1);
    return mergeCrossPatterns(series.dumps[idx], series.cross_patterns);
  }, [series, selectedDump]);

  const sourceName = useMemo(() => {
    if (dumpNames.length === 0) return "";
    if (dumpNames.length === 1) return dumpNames[0];
    const idx = Math.min(selectedDump, dumpNames.length - 1);
    return `${dumpNames[idx]} (+${dumpNames.length - 1})`;
  }, [dumpNames, selectedDump]);

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

  const runAnalysisSeries = useCallback(
    async (items: { text: string; name: string }[]) => {
      if (items.length === 0) return;
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
        const result = await analyzeMany(items.map((i) => i.text));
        setSeries(result);
        setDumpNames(items.map((i) => i.name));
        setSelectedDump(result.dumps.length - 1);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setSeries(null);
        setDumpNames([]);
      } finally {
        setBusyPhase(null);
      }
    },
    [wasmReady],
  );

  const onFiles = useCallback(
    async (fileList: FileList | File[]) => {
      const files = Array.from(fileList);
      if (files.length === 0) return;
      const items = await Promise.all(
        files.map(async (file) => ({
          text: await file.text(),
          name: file.name,
        })),
      );
      await runAnalysisSeries(items);
    },
    [runAnalysisSeries],
  );

  const [dragging, setDragging] = useState(false);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      const list = e.dataTransfer.files;
      if (list && list.length > 0) void onFiles(list);
    },
    [onFiles],
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
            multiple
            accept=".txt,.log,.tdump,.dump,text/plain"
            hidden
            disabled={busy}
            onChange={(e) => {
              const list = e.target.files;
              if (list && list.length > 0) void onFiles(list);
              e.target.value = "";
            }}
          />
        </label>
        <button
          className="btn"
          onClick={() =>
            void runAnalysisSeries([{ text: SAMPLE_DUMP, name: "sample.txt" }])
          }
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
      {!analysis && !busy && <p className="hint">{t("app.hint")}</p>}

      {series && series.dumps.length > 1 && (
        <section
          className="dump-series"
          data-testid="dump-series"
          aria-label={t("app.dumpSeries")}
        >
          <span className="dump-series-label">{t("app.dumpSeries")}</span>
          <div className="dump-series-chips">
            {series.dumps.map((d, i) => (
              <button
                key={`${dumpNames[i] ?? i}-${i}`}
                type="button"
                className={`dump-chip${selectedDump === i ? " active" : ""}`}
                data-testid={`dump-chip-${i}`}
                onClick={() => setSelectedDump(i)}
              >
                {t("app.dumpChip", {
                  name: dumpNames[i] ?? `dump-${i + 1}`,
                  count: d.total_threads,
                })}
              </button>
            ))}
          </div>
        </section>
      )}

      {error && (
        <p className="error" role="alert">
          {t("app.errorPrefix")}: {error}
        </p>
      )}

      {analysis && <Results analysis={analysis} />}

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
                  <option value="finalizer-pressure">
                    {t("codegen.finalizerPressure")}
                  </option>
                  <option value="sleep-as-scheduler">
                    {t("codegen.sleepAsScheduler")}
                  </option>
                  <option value="framework-pool-saturation">
                    {t("codegen.frameworkPoolSaturation")}
                  </option>
                  <option value="dns-resolution-stall">
                    {t("codegen.dnsResolutionStall")}
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
