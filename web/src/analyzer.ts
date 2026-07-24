import init, { analyzeDump, generateJava as wasmGenerateJava } from "./wasm/jblock";
import type { Analysis } from "./types";

export type JavaScenario = "lock-contention" | "deadlock";

let readyPromise: Promise<unknown> | null = null;

/** Lazily initialise the WASM module exactly once. */
export function ensureReady(): Promise<unknown> {
  if (!readyPromise) {
    readyPromise = init();
  }
  return readyPromise;
}

/** Parse and analyze a raw thread dump string using the Rust/WASM core. */
export async function analyze(text: string): Promise<Analysis> {
  await ensureReady();
  return analyzeDump(text) as Analysis;
}

/**
 * Generate a runnable Java reproducer for a thread problem scenario
 * (feat-007). Returns Java source; `count` is clamped to 2..=64 by the core.
 */
export async function generateJava(
  scenario: JavaScenario,
  count: number,
): Promise<string> {
  await ensureReady();
  return wasmGenerateJava(scenario, count);
}
