import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { gcRegroup, importTexts, isWasmReady, preloadWasm } from "./analyzer";
import {
  CLIPBOARD_DUMP_NAME,
  clipboardTextIsEmpty,
  isEditablePasteTarget,
  queryClipboardReadState,
  readSystemClipboard,
  shouldAutoReadClipboard,
  stripBom,
  textFromClipboardData,
} from "./clipboardImport";
import { exportHtml } from "./export";
import GcResults from "./GcResults";
import type { GcReport, GroupingMode } from "./gcTypes";
import GitHubLink from "./GitHubLink";
import HelpModal, { HelpButton } from "./HelpModal";
import { useI18n } from "./i18n";
import {
  appendItems,
  filesFromEntries,
  readImportBatch,
  type EntryLike,
  type FileLike,
  type ImportItem,
} from "./importFiles";
import LanguageMenu from "./LanguageMenu";
import PasteDumpModal from "./PasteDumpModal";
import Results from "./Results";
import { SAMPLE_DUMP } from "./sampleDump";
import { sha256Hex } from "./sha256";
import type { Analysis, MultiDumpAnalysis, PatternHit } from "./types";

type BusyPhase = "wasm" | "analyzing";
type View = "thread" | "gc";

/** Folder picker attribute that React does not type. */
const FOLDER_INPUT_PROPS = { webkitdirectory: "", directory: "" } as Record<string, string>;

function mergeCrossPatterns(dump: Analysis, cross: PatternHit[]): Analysis {
  if (cross.length === 0) return dump;
  return { ...dump, patterns: [...cross, ...dump.patterns] };
}

export default function App() {
  const { t, locale } = useI18n();
  const [series, setSeries] = useState<MultiDumpAnalysis | null>(null);
  const [dumpNames, setDumpNames] = useState<string[]>([]);
  const [dumpDigests, setDumpDigests] = useState<string[]>([]);
  const [selectedDump, setSelectedDump] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busyPhase, setBusyPhase] = useState<BusyPhase | null>(null);
  const [wasmReady, setWasmReady] = useState(() => isWasmReady());
  const fileInputRef = useRef<HTMLInputElement>(null);
  const busy = busyPhase !== null;

  // feat-074: the current import batch (kept for "Add files"), GC session report and view.
  const [items, setItems] = useState<ImportItem[]>([]);
  const [gcReport, setGcReport] = useState<GcReport | null>(null);
  const [gcNames, setGcNames] = useState<string[]>([]);
  const [gcRegrouping, setGcRegrouping] = useState(false);
  const [unknownNames, setUnknownNames] = useState<string[]>([]);
  const [skippedNames, setSkippedNames] = useState<string[]>([]);
  const [view, setView] = useState<View>("thread");

  const [helpOpen, setHelpOpen] = useState(false);
  const [pasteOpen, setPasteOpen] = useState(false);
  const pasteOpenRef = useRef(false);
  const helpOpenRef = useRef(false);
  const busyRef = useRef(false);

  const analysis = useMemo(() => {
    if (!series || series.dumps.length === 0) return null;
    const idx = Math.min(selectedDump, series.dumps.length - 1);
    return mergeCrossPatterns(series.dumps[idx], series.cross_patterns);
  }, [series, selectedDump]);

  const hasResults = analysis !== null || gcReport !== null;
  const showGc = gcReport !== null && (view === "gc" || analysis === null);

  const sourceName = useMemo(() => {
    if (dumpNames.length === 0) return "";
    if (dumpNames.length === 1) return dumpNames[0];
    const idx = Math.min(selectedDump, dumpNames.length - 1);
    return `${dumpNames[idx]} (+${dumpNames.length - 1})`;
  }, [dumpNames, selectedDump]);

  const selectedDumpName = useMemo(() => {
    if (dumpNames.length === 0) return "";
    return dumpNames[Math.min(selectedDump, dumpNames.length - 1)] ?? "";
  }, [dumpNames, selectedDump]);

  const selectedDigest = useMemo(() => {
    if (dumpDigests.length === 0) return "";
    return dumpDigests[Math.min(selectedDump, dumpDigests.length - 1)] ?? "";
  }, [dumpDigests, selectedDump]);

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
  const closePaste = useCallback(() => setPasteOpen(false), []);

  const runImport = useCallback(
    async (batch: ImportItem[], skipped: string[] = []) => {
      if (batch.length === 0) {
        if (skipped.length > 0) setSkippedNames(skipped);
        return;
      }
      setError(null);
      setBusyPhase(isWasmReady() || wasmReady ? "analyzing" : "wasm");
      try {
        if (!isWasmReady()) {
          setBusyPhase("wasm");
          await preloadWasm();
          setWasmReady(true);
        }
        setBusyPhase("analyzing");
        // Yield so the overlay paints before posting work to the analyze worker.
        await new Promise<void>((resolve) => {
          window.setTimeout(resolve, 0);
        });
        const result = await importTexts(batch, "auto");
        const threadBatch = result.threadItems.map((i) => batch[i]);
        const digests = await Promise.all(threadBatch.map((i) => sha256Hex(i.text)));
        setItems(batch);
        setSeries(result.thread);
        setDumpNames(threadBatch.map((i) => i.name));
        setDumpDigests(digests);
        setSelectedDump(result.thread ? result.thread.dumps.length - 1 : 0);
        setGcReport(result.gc);
        setGcNames(result.gcItems.map((i) => batch[i].name));
        setUnknownNames(
          result.kinds.flatMap((k, i) =>
            !k.thread_dump && !k.gc_log && !result.threadItems.includes(i) ? [batch[i].name] : [],
          ),
        );
        setSkippedNames(skipped);
        setView(result.thread ? "thread" : "gc");
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setSeries(null);
        setDumpNames([]);
        setDumpDigests([]);
        setGcReport(null);
        setGcNames([]);
      } finally {
        setBusyPhase(null);
      }
    },
    [wasmReady],
  );

  const onRegroup = useCallback(async (mode: GroupingMode) => {
    setGcRegrouping(true);
    try {
      setGcReport(await gcRegroup(mode));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGcRegrouping(false);
    }
  }, []);

  const onFiles = useCallback(
    async (fileList: FileList | FileLike[], opts: { append?: boolean } = {}) => {
      const files = Array.from(fileList as ArrayLike<FileLike>);
      if (files.length === 0) return;
      setError(null);
      // Show loading before reading large files (file.text() can stall the UI).
      setBusyPhase(isWasmReady() || wasmReady ? "analyzing" : "wasm");
      await new Promise<void>((resolve) => {
        requestAnimationFrame(() => resolve());
      });
      try {
        const batch = await readImportBatch(files);
        const next = opts.append ? appendItems(items, batch.items) : batch.items;
        await runImport(next, batch.skipped);
        // After runImport, which clears errors when it starts.
        if (batch.gzipFailed.length > 0) {
          setError(t("app.gzipUnsupported", { names: batch.gzipFailed.join(", ") }));
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setBusyPhase(null);
      }
    },
    [items, runImport, wasmReady, t],
  );

  const analyzeClipboardText = useCallback(
    (raw: string) => {
      const text = stripBom(raw);
      if (clipboardTextIsEmpty(text)) {
        setError(t("app.clipboardEmpty"));
        return;
      }
      setPasteOpen(false);
      void runImport([{ text, name: CLIPBOARD_DUMP_NAME }]);
    },
    [runImport, t],
  );

  const onPasteClipboard = useCallback(async () => {
    if (busy) return;
    setError(null);
    // Do not call readText() while clipboard-read is "prompt": Chrome then
    // shows a tiny native "Paste" chip, and our dialog only appears next click.
    const state = await queryClipboardReadState(navigator.permissions);
    if (shouldAutoReadClipboard(state)) {
      const result = await readSystemClipboard(navigator.clipboard);
      if (result.ok) {
        analyzeClipboardText(result.text);
        return;
      }
    }
    setPasteOpen(true);
  }, [analyzeClipboardText, busy]);

  useEffect(() => {
    pasteOpenRef.current = pasteOpen;
  }, [pasteOpen]);

  useEffect(() => {
    helpOpenRef.current = helpOpen;
  }, [helpOpen]);

  useEffect(() => {
    busyRef.current = busy;
  }, [busy]);

  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      if (busyRef.current || helpOpenRef.current || pasteOpenRef.current) return;
      if (isEditablePasteTarget(e.target as Element | null)) return;
      const files = e.clipboardData?.files;
      if (files && files.length > 0) {
        e.preventDefault();
        void onFiles(files);
        return;
      }
      const data = e.clipboardData;
      const text = textFromClipboardData(
        data ? (type) => data.getData(type) : null,
      );
      if (clipboardTextIsEmpty(text)) return;
      e.preventDefault();
      analyzeClipboardText(text);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [analyzeClipboardText, onFiles]);

  const [dragging, setDragging] = useState(false);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragging(false);
      // Dropped folders are only reachable through the entries API.
      const entries = Array.from(e.dataTransfer.items ?? [])
        .map((item) => item.webkitGetAsEntry?.() as EntryLike | null)
        .filter((entry): entry is EntryLike => entry != null);
      if (entries.some((entry) => entry.isDirectory)) {
        void filesFromEntries(entries).then((files) => onFiles(files));
        return;
      }
      const list = e.dataTransfer.files;
      if (list && list.length > 0) {
        void onFiles(list);
        return;
      }
      const text = e.dataTransfer.getData("text/plain");
      if (!clipboardTextIsEmpty(text)) analyzeClipboardText(text);
    },
    [analyzeClipboardText, onFiles],
  );

  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragging(true);
  }, []);

  const onDragLeave = useCallback((e: React.DragEvent) => {
    if (e.currentTarget === e.target) setDragging(false);
  }, []);

  const chooseDumpControl = (
    <label
      className={`btn primary${busy ? " disabled" : ""}`}
      data-testid="choose-dump"
    >
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
  );

  const sampleControl = (
    <button
      type="button"
      className="btn"
      data-testid="load-sample"
      onClick={() => void runImport([{ text: SAMPLE_DUMP, name: "sample.txt" }])}
      disabled={busy}
    >
      {t("app.loadSample")}
    </button>
  );

  const chooseFolderControl = (
    <label
      className={`btn${busy ? " disabled" : ""}`}
      data-testid="choose-folder"
    >
      {t("app.chooseFolder")}
      <input
        type="file"
        multiple
        hidden
        disabled={busy}
        {...FOLDER_INPUT_PROPS}
        onChange={(e) => {
          const list = e.target.files;
          if (list && list.length > 0) void onFiles(list);
          e.target.value = "";
        }}
      />
    </label>
  );

  const addFilesControl = (
    <label
      className={`btn${busy ? " disabled" : ""}`}
      data-testid="add-files"
      title={t("app.addFilesHint")}
    >
      {t("app.addFiles")}
      <input
        type="file"
        multiple
        hidden
        disabled={busy}
        onChange={(e) => {
          const list = e.target.files;
          if (list && list.length > 0) void onFiles(list, { append: true });
          e.target.value = "";
        }}
      />
    </label>
  );

  const gcSampleControl = (
    <button
      type="button"
      className="btn"
      data-testid="load-gc-sample"
      onClick={() =>
        void import("./sampleGc.log?raw").then((m) => runImport([{ text: m.default, name: "sample-g1-gc.log" }]))
      }
      disabled={busy}
    >
      {t("app.loadGcSample")}
    </button>
  );

  const pasteClipboardControl = (
    <button
      type="button"
      className="btn"
      data-testid="paste-clipboard"
      title={t("app.pasteClipboardHint")}
      onClick={() => void onPasteClipboard()}
      disabled={busy}
    >
      {t("app.pasteClipboard")}
    </button>
  );

  return (
    <div
      className={`app${dragging ? " dragging" : ""}${hasResults ? " has-results" : ""}`}
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
          <a className="brand" href="#top" onClick={(e) => e.preventDefault()}>
            <img
              className="brand-mark"
              src={`${import.meta.env.BASE_URL}favicon.svg`}
              width={18}
              height={18}
              alt=""
              aria-hidden="true"
            />
            <span className="brand-name">jblock</span>
          </a>
          <div className="header-actions">
            <LanguageMenu />
            <HelpButton onClick={openHelp} />
            <GitHubLink />
          </div>
        </div>
      </header>

      <section
        className={`home-intro${hasResults ? " is-collapsed" : ""}`}
        data-testid="home-intro"
        aria-hidden={hasResults}
      >
        <div className="home-intro-inner">
          <p className="home-kicker">{t("home.kicker")}</p>
          <h1 className="home-title">
            <span className="home-title-brand">jblock</span>
            <span className="home-title-rest">{t("home.title")}</span>
          </h1>
          <p className="home-lead">{t("home.lead")}</p>
          <p className="home-privacy">{t("home.privacy")}</p>

          {!hasResults && (
            <div className="home-cta controls" data-testid="home-cta">
              {chooseDumpControl}
              {chooseFolderControl}
              {pasteClipboardControl}
              {sampleControl}
              {gcSampleControl}
            </div>
          )}

          <ul className="home-points" aria-label={t("home.pointsLabel")}>
            <li>{t("home.pointLocal")}</li>
            <li>{t("home.pointFormats")}</li>
          </ul>
        </div>
      </section>

      {hasResults && (
        <section
          className="workspace-toolbar controls"
          data-testid="workspace-toolbar"
          aria-label={t("home.toolbarLabel")}
        >
          {chooseDumpControl}
          {addFilesControl}
          {chooseFolderControl}
          {pasteClipboardControl}
          {!showGc && analysis && (
            <button
              type="button"
              className="btn"
              onClick={() =>
                exportHtml(analysis, sourceName, t, locale, selectedDigest)
              }
              disabled={busy}
            >
              {t("app.exportHtml")}
            </button>
          )}
          {!showGc && selectedDumpName && (
            <span
              className="dump-filename"
              data-testid="dump-filename"
              title={
                selectedDigest
                  ? t("app.sha256", { hash: selectedDigest })
                  : undefined
              }
            >
              {selectedDumpName}
            </span>
          )}
        </section>
      )}

      {analysis && gcReport && (
        <div
          className="segmented view-switch"
          role="radiogroup"
          aria-label={t("app.viewSwitch")}
          data-testid="view-switch"
        >
          <button
            type="button"
            role="radio"
            aria-checked={!showGc}
            className={`segmented-option${!showGc ? " active" : ""}`}
            data-testid="view-thread"
            onClick={() => setView("thread")}
          >
            {t("app.viewThreads", { count: series?.dumps.length ?? 0 })}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={showGc}
            className={`segmented-option${showGc ? " active" : ""}`}
            data-testid="view-gc"
            onClick={() => setView("gc")}
          >
            {t("app.viewGc", { count: gcNames.length })}
          </button>
        </div>
      )}

      {(unknownNames.length > 0 || skippedNames.length > 0) && (
        <div className="import-warnings" data-testid="import-warnings">
          {unknownNames.length > 0 && (
            <p className="import-warning" data-testid="import-unrecognized">
              {t("app.unrecognized", {
                count: unknownNames.length,
                names: unknownNames.join(", "),
              })}
            </p>
          )}
          {skippedNames.length > 0 && (
            <p className="import-warning" data-testid="import-skipped">
              {t("app.skippedBinary", {
                count: skippedNames.length,
                names: skippedNames.join(", "),
              })}
            </p>
          )}
        </div>
      )}

      {!showGc && series && series.dumps.length > 1 && (
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

      {showGc && gcReport && (
        <GcResults
          key={gcNames.join("\n")}
          report={gcReport}
          names={gcNames}
          onRegroup={(mode) => void onRegroup(mode)}
          busy={busy || gcRegrouping}
        />
      )}

      {!showGc && analysis && (
        <Results
          key={`${selectedDumpName}:${selectedDigest || selectedDump}`}
          analysis={analysis}
        />
      )}

      {helpOpen && <HelpModal onClose={closeHelp} />}
      {pasteOpen && (
        <PasteDumpModal
          onClose={closePaste}
          onAnalyze={analyzeClipboardText}
        />
      )}
    </div>
  );
}
