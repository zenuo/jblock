#!/usr/bin/env node
/**
 * Unit checks for web/src/flamegraph.ts (Node --experimental-strip-types).
 */
import assert from "node:assert/strict";
import {
  buildFlameTree,
  layoutFlame,
  nodeAtPath,
  shortFrameLabel,
} from "../web/src/flamegraph.ts";
import { reportSections, sectionDomId } from "../web/src/reportNav.ts";

const threads = [
  {
    name: "a",
    state: "RUNNABLE",
    stack: ["pkg.Leaf.a(Leaf.java:1)", "pkg.Root.run(Root.java:2)"],
  },
  {
    name: "b",
    state: "RUNNABLE",
    stack: ["pkg.Leaf.b(Leaf.java:3)", "pkg.Root.run(Root.java:2)"],
  },
  {
    name: "c",
    state: "BLOCKED",
    stack: [],
  },
];

const tree = buildFlameTree(threads, "(no stack)");
assert.equal(tree.name, "all");
assert.equal(tree.value, 3);
assert.equal(
  tree.children.some((c) => c.name === "RUNNABLE" || c.name === "BLOCKED"),
  false,
  "default grouping is by stack, not thread state",
);

const rootRun = tree.children.find((c) => c.name.startsWith("pkg.Root.run"));
assert.ok(rootRun);
assert.equal(rootRun.value, 2);
assert.equal(rootRun.children.length, 2);
const empty = tree.children.find((c) => c.name === "(no stack)");
assert.ok(empty);
assert.equal(empty.value, 1);

const { rects, total, height } = layoutFlame(tree, 1000);
assert.equal(total, 3);
assert.ok(height >= 20);
const allRect = rects.find((r) => r.name === "all");
assert.ok(allRect);
assert.equal(allRect.width, 1000);
assert.equal(allRect.x, 0);
const rootRunRect = rects.find((r) => r.name.startsWith("pkg.Root.run"));
assert.ok(rootRunRect);
assert.ok(rootRunRect.y < allRect.y);
const leaf = rects.find((r) => r.name.startsWith("pkg.Leaf.a"));
assert.ok(leaf);
assert.ok(leaf.y < rootRunRect.y);

const byState = buildFlameTree(threads, "(no stack)", "state");
const runnable = byState.children.find((c) => c.name === "RUNNABLE");
const blocked = byState.children.find((c) => c.name === "BLOCKED");
assert.ok(runnable);
assert.ok(blocked);
assert.equal(runnable.value, 2);
assert.equal(blocked.value, 1);
assert.equal(blocked.children[0]?.name, "(no stack)");
const stateRootRun = runnable.children.find((c) =>
  c.name.startsWith("pkg.Root.run"),
);
assert.ok(stateRootRun);
assert.equal(stateRootRun.value, 2);
assert.equal(stateRootRun.children.length, 2);

const stateLayout = layoutFlame(byState, 1000);
const runRect = stateLayout.rects.find((r) => r.name === "RUNNABLE");
const blockRect = stateLayout.rects.find((r) => r.name === "BLOCKED");
assert.ok(runRect && blockRect);
assert.ok(Math.abs(runRect.width - (2 / 3) * 1000) < 0.001);
assert.ok(Math.abs(blockRect.width - (1 / 3) * 1000) < 0.001);
const stateAll = stateLayout.rects.find((r) => r.name === "all");
assert.ok(stateAll);
assert.ok(runRect.y < stateAll.y);

const found = nodeAtPath(byState, ["all", "BLOCKED", "(no stack)"]);
assert.ok(found);
assert.equal(found.value, 1);
assert.equal(shortFrameLabel("com.foo.Bar.method(Bar.java:9)"), "Bar.method");

const withAll = reportSections({ hasDeadlocks: true, hasClusters: true });
assert.deepEqual(
  withAll.map((s) => s.id),
  [
    "findings",
    "deadlocks",
    "contention",
    "states",
    "clusters",
    "threads",
    "flamegraph",
  ],
);
assert.equal(withAll.at(-1)?.id, "flamegraph");
const slim = reportSections({ hasDeadlocks: false, hasClusters: false });
assert.deepEqual(
  slim.map((s) => s.id),
  ["findings", "contention", "states", "threads", "flamegraph"],
);
assert.equal(sectionDomId("findings"), "section-findings");

console.log("flamegraph unit tests ok");
