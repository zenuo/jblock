/**
 * Dedicated worker that owns the Rust/WASM analyzer (feat-050).
 *
 * Running analyzeDump / analyzeDumps off the UI thread keeps the loading
 * overlay responsive for large dumps.
 */

import init, { analyzeDump, analyzeDumps } from "./wasm/jblock";
import type {
  AnalyzeWorkerRequest,
  AnalyzeWorkerResponse,
} from "./analyzeMessages";

// Shared app tsconfig includes DOM; cast self for the worker global scope.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const workerScope = self as any;

let ready = false;

async function ensureInit(): Promise<void> {
  if (ready) return;
  await init();
  ready = true;
}

function reply(msg: AnalyzeWorkerResponse): void {
  workerScope.postMessage(msg);
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
