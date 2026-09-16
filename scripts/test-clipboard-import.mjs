#!/usr/bin/env node
import assert from "node:assert/strict";
import {
  CLIPBOARD_DUMP_NAME,
  clipboardTextIsEmpty,
  isEditablePasteTarget,
  readSystemClipboard,
  stripBom,
  textFromClipboardData,
} from "../web/src/clipboardImport.ts";

assert.equal(CLIPBOARD_DUMP_NAME, "clipboard.txt");
assert.equal(stripBom("\uFEFFhello"), "hello");
assert.equal(stripBom("hello"), "hello");
assert.equal(clipboardTextIsEmpty("  \n\t"), true);
assert.equal(clipboardTextIsEmpty("\uFEFF  "), true);
assert.equal(clipboardTextIsEmpty("dump"), false);

assert.equal(isEditablePasteTarget(null), false);
assert.equal(isEditablePasteTarget({ tagName: "TEXTAREA" }), true);
assert.equal(isEditablePasteTarget({ tagName: "INPUT" }), true);
assert.equal(isEditablePasteTarget({ tagName: "SELECT" }), true);
assert.equal(isEditablePasteTarget({ tagName: "DIV", isContentEditable: true }), true);
assert.equal(isEditablePasteTarget({ tagName: "DIV" }), false);
assert.equal(
  isEditablePasteTarget({
    tagName: "SPAN",
    closest: (sel) => (sel.includes("textarea") ? {} : null),
  }),
  true,
);
assert.equal(
  isEditablePasteTarget({
    tagName: "SPAN",
    closest: () => null,
  }),
  false,
);

assert.equal(textFromClipboardData((type) => (type === "text/plain" ? "stack" : "")), "stack");
assert.equal(textFromClipboardData((type) => (type === "text/plain" ? "\uFEFFstack" : "")), "stack");
assert.equal(textFromClipboardData(null), "");

const ok = await readSystemClipboard({
  readText: async () => "Full thread dump\n",
});
assert.deepEqual(ok, { ok: true, text: "Full thread dump\n" });

const empty = await readSystemClipboard({
  readText: async () => "  ",
});
assert.deepEqual(empty, { ok: false, reason: "empty" });

const denied = await readSystemClipboard({
  readText: async () => {
    throw new Error("NotAllowedError");
  },
});
assert.deepEqual(denied, { ok: false, reason: "denied" });

const missing = await readSystemClipboard(undefined);
assert.deepEqual(missing, { ok: false, reason: "unavailable" });

console.log("clipboardImport tests ok");
