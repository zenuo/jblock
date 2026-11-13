/**
 * Dedicated worker that owns the Rust/WASM analyzer (feat-050).
 *
 * Running analyzeDump / analyzeDumps off the UI thread keeps the loading
 * overlay responsive for large dumps. The GC session (feat-074) also lives
 * here so range stats, chart columns and event pages are computed without
 * shipping every GC event to the UI thread.
 */

import init, {
  GcSession,
  analyzeDump,
  analyzeDumps,
  detectInputKinds,
} from "./wasm/jblock";
import type {
  AnalyzeWorkerRequest,
  AnalyzeWorkerResponse,
  ImportResult,
} from "./analyzeMessages";
import type { GcReport, InputKinds } from "./gcTypes";
import type { MultiDumpAnalysis } from "./types";

// Shared app tsconfig includes DOM; cast self for the worker global scope.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const workerScope = self as any;

let ready = false;
let gcSession: GcSession | null = null;

async function ensureInit(): Promise<void> {
  if (ready) return;
  await init();
  ready = true;
}

function reply(msg: AnalyzeWorkerResponse, transfer: Transferable[] = []): void {
  workerScope.postMessage(msg, transfer);
}

function requireSession(): GcSession {
  if (!gcSession) throw new Error("no GC logs loaded");
  return gcSession;
}

workerScope.onmessage = (event: MessageEvent<AnalyzeWorkerRequest>) => {
  const msg = event.data;
  void (async () => {
    try {
      switch (msg.type) {
        case "init":
          await ensureInit();
          reply({ id: msg.id, type: "ok" });
          break;
        case "analyze": {
          await ensureInit();
          const result = analyzeDump(msg.text);
          reply({ id: msg.id, type: "ok", result });
          break;
        }
        case "analyzeMany": {
          await ensureInit();
          if (msg.texts.length === 0) {
            throw new Error("analyzeMany requires at least one dump");
          }
          const result = analyzeDumps(msg.texts);
          reply({ id: msg.id, type: "ok", result });
          break;
        }
        case "importTexts": {
          await ensureInit();
          const kinds = msg.items.map(
            (item) => detectInputKinds(item.text) as InputKinds,
          );
          const gcItems = kinds.flatMap((k, i) => (k.gc_log ? [i] : []));
          // Without any GC log in the batch, unrecognized texts still go to the
          // thread-dump analyzer best-effort, exactly as before feat-074.
          const threadItems = kinds.flatMap((k, i) =>
            k.thread_dump || (gcItems.length === 0 && !k.gc_log) ? [i] : [],
          );
          const thread =
            threadItems.length > 0
              ? (analyzeDumps(threadItems.map((i) => msg.items[i].text)) as MultiDumpAnalysis)
              : null;
          gcSession?.free();
          gcSession = null;
          let gc: GcReport | null = null;
          if (gcItems.length > 0) {
            gcSession = new GcSession(
              gcItems.map((i) => msg.items[i]),
              msg.mode,
            );
            gc = gcSession.report() as GcReport;
          }
          const result: ImportResult = { kinds, threadItems, thread, gcItems, gc };
          reply({ id: msg.id, type: "ok", result });
          break;
        }
        case "gcRegroup": {
          const session = requireSession();
          session.regroup(msg.mode);
          reply({ id: msg.id, type: "ok", result: session.report() });
          break;
        }
        case "gcSeries": {
          const session = requireSession();
          const columns: Record<string, Float64Array> = {};
          for (const name of GcSession.columnNames()) {
            columns[name] = session.column(msg.group, name);
          }
          reply(
            { id: msg.id, type: "ok", result: columns },
            Object.values(columns).map((c) => c.buffer),
          );
          break;
        }
        case "gcStats": {
          const result = requireSession().stats(msg.group, msg.t0, msg.t1);
          reply({ id: msg.id, type: "ok", result });
          break;
        }
        case "gcEvents": {
          const result = requireSession().events(msg.group, msg.query);
          reply({ id: msg.id, type: "ok", result });
          break;
        }
        default: {
          const _exhaustive: never = msg;
          throw new Error(`unknown worker request: ${JSON.stringify(_exhaustive)}`);
        }
      }
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e);
      reply({ id: msg.id, type: "error", error });
    }
  })();
};
