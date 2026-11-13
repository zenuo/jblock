/** Event table query state (feat-077). */

import type { GcKind } from "./gcTypes";

export interface TableQuery {
  kinds: GcKind[];
  cause: string;
  minDurationMs: string;
  pausesOnly: boolean;
  sort: "time" | "duration";
  desc: boolean;
}

export const DEFAULT_TABLE_QUERY: TableQuery = {
  kinds: [],
  cause: "",
  minDurationMs: "",
  pausesOnly: false,
  sort: "time",
  desc: false,
};
