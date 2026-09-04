#!/usr/bin/env node
/**
 * Unit checks for sortThreads id ordering (Node --experimental-strip-types).
 */
import assert from "node:assert/strict";
import { sortThreads } from "../web/src/analysisUi.ts";

function th(name, id) {
  return {
    name,
    id,
    state: "RUNNABLE",
    waiting_on: null,
    held_locks: [],
    stack_depth: 0,
    stack: [],
    kind: "platform",
    carrier_id: null,
    mounted_id: null,
  };
}

const mixed = [th("c", "100"), th("a", "9"), th("b", "10"), th("z", null)];

const asc = sortThreads(mixed, "id", "asc").map((t) => t.id);
assert.deepEqual(asc, ["9", "10", "100", null]);

const desc = sortThreads(mixed, "id", "desc").map((t) => t.id);
assert.deepEqual(desc, ["100", "10", "9", null]);

const names = sortThreads(mixed, "name", "asc").map((t) => t.name);
assert.deepEqual(names, ["a", "b", "c", "z"]);

console.log("sortThreads tests ok");
