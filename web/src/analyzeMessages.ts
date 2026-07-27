/** Shared message protocol between the main thread and the analyze Web Worker. */

export type AnalyzeWorkerRequest =
  | { id: number; type: "init" }
  | { id: number; type: "analyze"; text: string }
  | { id: number; type: "analyzeMany"; texts: string[] };

export type AnalyzeWorkerResponse =
  | { id: number; type: "ok"; result?: unknown }
  | { id: number; type: "error"; error: string };
