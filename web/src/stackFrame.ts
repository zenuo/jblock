export type StackTokenKind =
  | "pkg"
  | "cls"
  | "method"
  | "punct"
  | "file"
  | "line"
  | "native"
  | "text";

export interface StackToken {
  kind: StackTokenKind;
  text: string;
}

function tokenizeLocation(loc: string): StackToken[] {
  if (
    loc === "Native Method" ||
    loc === "Unknown Source" ||
    loc === "<generated>"
  ) {
    return [{ kind: "native", text: loc }];
  }
  const colon = loc.lastIndexOf(":");
  const line = colon >= 0 ? loc.slice(colon + 1) : "";
  if (colon > 0 && /^\d+$/.test(line)) {
    return [
      { kind: "file", text: loc.slice(0, colon) },
      { kind: "punct", text: ":" },
      { kind: "line", text: line },
    ];
  }
  return [{ kind: "file", text: loc }];
}

/** Split a jstack/MXBean frame (`pkg.Cls.method(File.java:12)`) into highlight tokens. */
export function tokenizeStackFrame(raw: string): StackToken[] {
  const frame = raw.trim().replace(/^at\s+/, "");
  if (!frame) return [];

  const open = frame.lastIndexOf("(");
  if (open < 0 || !frame.endsWith(")")) {
    return [{ kind: "text", text: frame }];
  }

  const qual = frame.slice(0, open);
  const loc = frame.slice(open + 1, -1);
  const tokens: StackToken[] = [];

  const lastDot = qual.lastIndexOf(".");
  if (lastDot < 0) {
    tokens.push({ kind: "method", text: qual });
  } else {
    const typeName = qual.slice(0, lastDot);
    const method = qual.slice(lastDot + 1);
    const typeDot = typeName.lastIndexOf(".");
    if (typeDot < 0) {
      if (typeName) tokens.push({ kind: "cls", text: typeName });
    } else {
      tokens.push({ kind: "pkg", text: typeName.slice(0, typeDot) });
      tokens.push({ kind: "punct", text: "." });
      tokens.push({ kind: "cls", text: typeName.slice(typeDot + 1) });
    }
    tokens.push({ kind: "punct", text: "." });
    tokens.push({ kind: "method", text: method });
  }

  tokens.push({ kind: "punct", text: "(" });
  tokens.push(...tokenizeLocation(loc));
  tokens.push({ kind: "punct", text: ")" });
  return tokens;
}

export function stackFrameHtml(
  frame: string,
  escape: (value: string) => string,
): string {
  return tokenizeStackFrame(frame)
    .map(
      (tok) =>
        `<span class="sf sf-${tok.kind}">${escape(tok.text)}</span>`,
    )
    .join("");
}
