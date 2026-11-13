/**
 * Main-thread facade for the WASM analyzer.
 *
 * Heavy parse/analyze work runs in `analyze.worker.ts` so large dumps do not
 * freeze the UI thread (feat-050). `analyzeDump` / `analyzeDumps` are invoked
 * only inside that worker.
 */

import type {
  AnalyzeWorkerRequest,
  AnalyzeWorkerResponse,
  ImportResult,
  NamedText,
} from "./analyzeMessages";
import type {
  EventPage,
  EventQuery,
  GcReport,
  GcSeries,
  GcStats,
  GroupingMode,
} from "./gcTypes";
import type { Analysis, MultiDumpAnalysis } from "./types";

export type { ImportResult, NamedText } from "./analyzeMessages";

export type { JavaScenario } from "./codegen";
export { generateJava, classNameFor } from "./codegen";

type Pending = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
};

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown
  ? Omit<T, K>
  : never;

type AnalyzeWorkerRequestBody = DistributiveOmit<AnalyzeWorkerRequest, "id">;

let worker: Worker | null = null;
let nextId = 1;
const pending = new Map<number, Pending>();
let readyPromise: Promise<unknown> | null = null;
let wasmReady = false;

function getWorker(): Worker {
  if (!worker) {
    worker = new Worker(new URL("./analyze.worker.ts", import.meta.url), {
      type: "module",
    });
    worker.onmessage = (event: MessageEvent<AnalyzeWorkerResponse>) => {
      const msg = event.data;
      const entry = pending.get(msg.id);
      if (!entry) return;
      pending.delete(msg.id);
      if (msg.type === "ok") {
        entry.resolve(msg.result);
      } else {
        entry.reject(new Error(msg.error));
      }
    };
    worker.onerror = (event) => {
      const err = new Error(event.message || "analyze worker failed");
      for (const [, entry] of pending) {
        entry.reject(err);
      }
      pending.clear();
      readyPromise = null;
      wasmReady = false;
      worker = null;
    };
  }
  return worker;
}

function callWorker(request: AnalyzeWorkerRequestBody): Promise<unknown> {
  const id = nextId++;
  const full = { ...request, id } as AnalyzeWorkerRequest;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    try {
      getWorker().postMessage(full);
    } catch (e) {
      pending.delete(id);
      reject(e);
    }
  });
}

/** True once the worker has finished initialising the WASM module. */
export function isWasmReady(): boolean {
  return wasmReady;
}

/**
 * Initialise the WASM module inside the worker exactly once.
 * Safe to call on page load for background preload, and again before analyze.
 */
export function ensureReady(): Promise<unknown> {
  if (!readyPromise) {
    readyPromise = callWorker({ type: "init" }).then((exports) => {
      wasmReady = true;
      return exports;
    });
  }
  return readyPromise;
}

/** Kick off WASM load without waiting (same as ensureReady, clearer at call sites). */
export function preloadWasm(): Promise<unknown> {
  return ensureReady();
}

/** Parse and analyze a raw thread dump string using the Rust/WASM core (in a worker). */
export async function analyze(text: string): Promise<Analysis> {
  await ensureReady();
  return (await callWorker({ type: "analyze", text })) as Analysis;
}

/** Analyze an ordered series of dumps (cross-dump leak/livelock, feat-041). */
export async function analyzeMany(texts: string[]): Promise<MultiDumpAnalysis> {
  await ensureReady();
  if (texts.length === 0) {
    throw new Error("analyzeMany requires at least one dump");
  }
  return (await callWorker({
    type: "analyzeMany",
    texts,
  })) as MultiDumpAnalysis;
}

/**
 * Classify a batch of texts and analyze each kind (feat-074): thread dumps go
 * through `analyzeDumps`, GC logs open a new worker-side GC session.
 */
export async function importTexts(
  items: NamedText[],
  mode: GroupingMode = "auto",
): Promise<ImportResult> {
  await ensureReady();
  return (await callWorker({ type: "importTexts", items, mode })) as ImportResult;
}

/** Regroup the current GC session's files (auto / merge / split). */
export async function gcRegroup(mode: GroupingMode): Promise<GcReport> {
  return (await callWorker({ type: "gcRegroup", mode })) as GcReport;
}

/** Chart columns for one GC group (Float64Arrays transferred from the worker). */
export async function gcSeries(group: number): Promise<GcSeries> {
  return (await callWorker({ type: "gcSeries", group })) as GcSeries;
}

/** KPIs for a time range on the group's time axis. */
export async function gcStats(
  group: number,
  t0: number,
  t1: number,
): Promise<GcStats> {
  return (await callWorker({ type: "gcStats", group, t0, t1 })) as GcStats;
}

/** One page of GC events. */
export async function gcEvents(
  group: number,
  query: EventQuery,
): Promise<EventPage> {
  return (await callWorker({ type: "gcEvents", group, query })) as EventPage;
}
