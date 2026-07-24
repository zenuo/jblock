import init, { analyzeDump } from "./wasm/jblock";
import type { Analysis } from "./types";

export type { JavaScenario } from "./codegen";
export { generateJava, classNameFor } from "./codegen";

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
