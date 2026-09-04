#!/usr/bin/env node
import assert from "node:assert/strict";
import { tokenizeStackFrame } from "../web/src/stackFrame.ts";

function kinds(frame) {
  return tokenizeStackFrame(frame).map((t) => `${t.kind}:${t.text}`);
}

assert.deepEqual(
  kinds("java.net.SocketInputStream.socketRead0(Native Method)"),
  [
    "pkg:java.net",
    "punct:.",
    "cls:SocketInputStream",
    "punct:.",
    "method:socketRead0",
    "punct:(",
    "native:Native Method",
    "punct:)",
  ],
);

assert.deepEqual(
  kinds("com.mysql.jdbc.MysqlIO.readFully(MysqlIO.java:2966)"),
  [
    "pkg:com.mysql.jdbc",
    "punct:.",
    "cls:MysqlIO",
    "punct:.",
    "method:readFully",
    "punct:(",
    "file:MysqlIO.java",
    "punct::",
    "line:2966",
    "punct:)",
  ],
);

assert.deepEqual(
  kinds("Foo.bar(<generated>)"),
  ["cls:Foo", "punct:.", "method:bar", "punct:(", "native:<generated>", "punct:)"],
);

assert.equal(tokenizeStackFrame("not a frame")[0].kind, "text");

console.log("stackFrame tests ok");
