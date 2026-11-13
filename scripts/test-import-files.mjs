#!/usr/bin/env node
// Unit tests for web/src/importFiles.ts (feat-074): binary skip, directory walk, append.
import assert from "node:assert/strict";
import {
  appendItems,
  compareNames,
  filesFromEntries,
  isIgnoredName,
  looksBinary,
  readImportBatch,
} from "../web/src/importFiles.ts";

const enc = new TextEncoder();

function fakeFile(name, content, webkitRelativePath = "") {
  const bytes = typeof content === "string" ? enc.encode(content) : content;
  return {
    name,
    webkitRelativePath,
    slice: (s = 0, e = bytes.length) => ({ arrayBuffer: async () => bytes.slice(s, e).buffer }),
    text: async () => new TextDecoder().decode(bytes),
  };
}

function fileEntry(path, file) {
  const name = path.split("/").pop();
  return { isFile: true, isDirectory: false, name, fullPath: `/${path}`, file: (ok) => ok(file) };
}

function dirEntry(path, children, batch = 2) {
  const name = path.split("/").pop();
  return {
    isFile: false,
    isDirectory: true,
    name,
    fullPath: `/${path}`,
    createReader() {
      let i = 0;
      // Return entries in small batches like Chrome does, then an empty batch.
      return { readEntries: (ok) => { const b = children.slice(i, i + batch); i += batch; ok(b); } };
    },
  };
}

// Binary detection and ignored names.
assert.equal(looksBinary(enc.encode("[0.001s][info][gc] Using G1\n")), false);
assert.equal(looksBinary(new Uint8Array([0x1f, 0x8b, 0x08, 0x00])), true);
assert.equal(isIgnoredName(".DS_Store"), true);
assert.equal(isIgnoredName("logs/.hidden"), true);
assert.equal(isIgnoredName("gc.log"), false);
assert.ok(compareNames("gc.log.2", "gc.log.10") < 0);

// readImportBatch: natural order, BOM stripped, binary skipped, relative paths kept.
const batch = await readImportBatch([
  fakeFile("gc.log.10", "ten"),
  fakeFile("gc.log.2", "﻿two", "logs/gc.log.2"),
  fakeFile("heap.hprof", new Uint8Array([0x4a, 0x41, 0x56, 0x41, 0x00, 0x01])),
  fakeFile(".DS_Store", "x"),
]);
assert.deepEqual(batch.items.map((i) => i.name), ["gc.log.10", "logs/gc.log.2"].sort((a, b) => compareNames(a, b)));
assert.equal(batch.items.find((i) => i.name === "logs/gc.log.2").text, "two");
assert.deepEqual(batch.skipped, ["heap.hprof"]);

// Directory walk across batched readEntries, nested folders, hidden files ignored.
const tree = [
  dirEntry("node-a", [
    fileEntry("node-a/gc.log.0", fakeFile("gc.log.0", "a0")),
    fileEntry("node-a/gc.log.1", fakeFile("gc.log.1", "a1")),
    fileEntry("node-a/.DS_Store", fakeFile(".DS_Store", "x")),
    dirEntry("node-a/old", [fileEntry("node-a/old/gc.log", fakeFile("gc.log", "old"))]),
  ]),
  fileEntry("dump.txt", fakeFile("dump.txt", "dump")),
];
const walked = await filesFromEntries(tree);
const names = walked.map((f) => f.webkitRelativePath || f.name).sort(compareNames);
assert.deepEqual(names, ["dump.txt", "node-a/gc.log.0", "node-a/gc.log.1", "node-a/old/gc.log"]);
assert.equal(await walked.find((f) => f.webkitRelativePath === "node-a/old/gc.log").text(), "old");

// appendItems: exact duplicates dropped, same name with new content renamed.
const appended = appendItems(
  [{ name: "gc.log", text: "a" }],
  [{ name: "gc.log", text: "a" }, { name: "gc.log", text: "b" }, { name: "x", text: "c" }],
);
assert.deepEqual(appended.map((i) => i.name), ["gc.log", "gc.log (2)", "x"]);

console.log("import-files: all assertions passed");

// feat-082: gzip inputs are decompressed transparently (Node has DecompressionStream).
import { gzipSync } from "node:zlib";
import { isGzip } from "../web/src/importFiles.ts";
const gz = new Uint8Array(gzipSync(enc.encode("[0.001s][info][gc] Using G1\n"))); // own buffer (Node Buffers share a pool)
assert.equal(isGzip(gz), true);
const gzFile = { ...fakeFile("gc.log.3.gz", gz), stream: () => new Blob([gz]).stream() };
const corrupt = { ...fakeFile("bad.gz", new Uint8Array([0x1f, 0x8b, 0x08, 0, 1, 2, 3])), stream: () => new Blob([new Uint8Array([0x1f, 0x8b, 0x08, 0, 1, 2, 3])]).stream() };
const zipped = await readImportBatch([gzFile, corrupt]);
assert.deepEqual(zipped.items, [{ name: "gc.log.3", text: "[0.001s][info][gc] Using G1\n" }]);
assert.deepEqual(zipped.gzipFailed, ["bad.gz"]);
assert.deepEqual(zipped.skipped, [], "gzip is not treated as an unknown binary");
console.log("import-files (gzip): all assertions passed");
