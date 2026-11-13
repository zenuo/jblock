/** Shared message protocol between the main thread and the analyze Web Worker. */

import type { EventQuery, GcReport, GroupingMode, InputKinds } from "./gcTypes";
import type { MultiDumpAnalysis } from "./types";

export interface NamedText {
  name: string;
  text: string;
}

/** Result of classifying and analyzing a mixed batch of inputs (feat-074). */
export interface ImportResult {
  kinds: InputKinds[];
  /** Indices (into the request items) analyzed as thread dumps. */
  threadItems: number[];
  thread: MultiDumpAnalysis | null;
  /** Indices (into the request items) analyzed as GC logs; GcReport file i = gcItems[i]. */
  gcItems: number[];
  gc: GcReport | null;
}

export type AnalyzeWorkerRequest =
  | { id: number; type: "init" }
  | { id: number; type: "analyze"; text: string }
  | { id: number; type: "analyzeMany"; texts: string[] }
  | { id: number; type: "importTexts"; items: NamedText[]; mode: GroupingMode }
  | { id: number; type: "gcRegroup"; mode: GroupingMode }
  | { id: number; type: "gcSeries"; group: number }
  | { id: number; type: "gcStats"; group: number; t0: number; t1: number }
  | { id: number; type: "gcEvents"; group: number; query: EventQuery };

export type AnalyzeWorkerResponse =
  | { id: number; type: "ok"; result?: unknown }
  | { id: number; type: "error"; error: string };
