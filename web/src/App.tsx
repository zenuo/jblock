import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { analyzeMany, isWasmReady, preloadWasm } from "./analyzer";
import { exportHtml, exportPdf } from "./export";
import HelpModal, { HelpButton } from "./HelpModal";
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

  const [helpOpen, setHelpOpen] = useState(false);

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

  const closeHelp = useCallback(() => setHelpOpen(false), []);
  const openHelp = useCallback(() => setHelpOpen(true), []);

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
            <HelpButton onClick={openHelp} />
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

      {helpOpen && <HelpModal onClose={closeHelp} />}
    </div>
  );
}
