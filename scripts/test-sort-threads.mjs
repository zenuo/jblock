#!/usr/bin/env node
/**
 * Unit checks for sortThreads (Node --experimental-strip-types).
 */
import assert from "node:assert/strict";
import {
  applySortClick,
  sortThreads,
  threadHasWaitingOn,
} from "../web/src/analysisUi.ts";

function th(name, id, extra = {}) {
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
    ...extra,
  };
}

const mixed = [th("c", "100"), th("a", "9"), th("b", "10"), th("z", null)];

const asc = sortThreads(mixed, [{ key: "id", dir: "asc" }]).map((t) => t.id);
assert.deepEqual(asc, ["9", "10", "100", null]);

const desc = sortThreads(mixed, [{ key: "id", dir: "desc" }]).map((t) => t.id);
assert.deepEqual(desc, ["100", "10", "9", null]);

const names = sortThreads(mixed, [{ key: "name", dir: "asc" }]).map((t) => t.name);
assert.deepEqual(names, ["a", "b", "c", "z"]);

const combo = [
  th("b", "1", { state: "WAITING" }),
  th("a", "2", { state: "WAITING" }),
  th("c", "3", { state: "RUNNABLE" }),
];
const comboNames = sortThreads(combo, [
  { key: "state", dir: "asc" },
  { key: "name", dir: "asc" },
]).map((t) => t.name);
assert.deepEqual(comboNames, ["c", "a", "b"]);

assert.equal(threadHasWaitingOn(th("x", "1")), false);
assert.equal(threadHasWaitingOn(th("x", "1", { waiting_on: "0x1" })), true);
assert.equal(threadHasWaitingOn(th("x", "1", { waiting_on: "  " })), false);

assert.deepEqual(applySortClick([{ key: "name", dir: "asc" }], "id", false), [
  { key: "id", dir: "asc" },
]);
assert.deepEqual(applySortClick([{ key: "name", dir: "asc" }], "name", false), [
  { key: "name", dir: "desc" },
]);
assert.deepEqual(applySortClick([{ key: "name", dir: "asc" }], "id", true), [
  { key: "name", dir: "asc" },
  { key: "id", dir: "asc" },
]);
assert.deepEqual(
  applySortClick(
    [
      { key: "name", dir: "asc" },
      { key: "id", dir: "asc" },
    ],
    "id",
    true,
  ),
  [
    { key: "name", dir: "asc" },
    { key: "id", dir: "desc" },
  ],
);
assert.deepEqual(
  applySortClick(
    [
      { key: "name", dir: "asc" },
      { key: "id", dir: "asc" },
    ],
    "state",
    false,
  ),
  [{ key: "state", dir: "asc" }],
);

console.log("sortThreads tests ok");
