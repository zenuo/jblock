import init, { analyzeDump, analyzeDumps } from "./wasm/jblock";
import type { Analysis, MultiDumpAnalysis } from "./types";

export type { JavaScenario } from "./codegen";
export { generateJava, classNameFor } from "./codegen";

let readyPromise: Promise<unknown> | null = null;
let wasmReady = false;

/** True once the WASM module has finished initialising. */
export function isWasmReady(): boolean {
  return wasmReady;
}

/**
 * Initialise the WASM module exactly once.
 * Safe to call on page load for background preload, and again before analyze.
 */
export function ensureReady(): Promise<unknown> {
  if (!readyPromise) {
    readyPromise = init().then((exports) => {
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

/** Parse and analyze a raw thread dump string using the Rust/WASM core. */
export async function analyze(text: string): Promise<Analysis> {
  await ensureReady();
  return analyzeDump(text) as Analysis;
}

/** Analyze an ordered series of dumps (cross-dump leak/livelock, feat-041). */
export async function analyzeMany(texts: string[]): Promise<MultiDumpAnalysis> {
  await ensureReady();
  if (texts.length === 0) {
    throw new Error("analyzeMany requires at least one dump");
  }
  return analyzeDumps(texts) as MultiDumpAnalysis;
}
