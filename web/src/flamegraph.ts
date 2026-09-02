/**
 * Thread-dump flame graph: each thread is one sample. Stacks are reversed so
 * the oldest frame sits nearest the root (bottom of the chart).
 */

export interface FlameNode {
  name: string;
  value: number;
  children: FlameNode[];
}

export interface FlameRect {
  name: string;
  value: number;
  x: number;
  y: number;
  width: number;
  height: number;
  depth: number;
  path: string[];
}

export const FLAME_ROW_HEIGHT = 20;
export const FLAME_MAX_FRAMES = 40;
const MIN_RECT_WIDTH = 0.5;
const LABEL_MIN_WIDTH = 36;

const STATE_COLORS: Record<string, string> = {
  RUNNABLE: "#22c55e",
  BLOCKED: "#ef4444",
  WAITING: "#f59e0b",
  TIMED_WAITING: "#eab308",
  NEW: "#38bdf8",
  TERMINATED: "#94a3b8",
};

export function defaultStateColors(): Record<string, string> {
  return { ...STATE_COLORS };
}

export interface FlameThread {
  name: string;
  state: string;
  stack: string[];
}

export function buildFlameTree(
  threads: FlameThread[],
  emptyFrame = "(no stack)",
): FlameNode {
  const root: FlameNode = { name: "all", value: 0, children: [] };
  const index = new Map<FlameNode, Map<string, FlameNode>>();

  const childOf = (parent: FlameNode, name: string): FlameNode => {
    let map = index.get(parent);
    if (!map) {
      map = new Map();
      index.set(parent, map);
    }
    let node = map.get(name);
    if (!node) {
      node = { name, value: 0, children: [] };
      map.set(name, node);
      parent.children.push(node);
    }
    return node;
  };

  for (const thread of threads) {
    root.value += 1;
    let node = childOf(root, thread.state || "UNKNOWN");
    node.value += 1;
    const frames =
      thread.stack.length > 0
        ? [...thread.stack].reverse().slice(0, FLAME_MAX_FRAMES)
        : [emptyFrame];
    for (const frame of frames) {
      node = childOf(node, frame);
      node.value += 1;
    }
  }

  const sortTree = (node: FlameNode) => {
    node.children.sort(
      (a, b) => b.value - a.value || a.name.localeCompare(b.name),
    );
    for (const child of node.children) sortTree(child);
  };
  sortTree(root);
  return root;
}

export function nodeAtPath(root: FlameNode, path: string[]): FlameNode | null {
  if (path.length === 0) return root;
  let node: FlameNode = root;
  const start = path[0] === root.name ? 1 : 0;
  for (let i = start; i < path.length; i++) {
    const next = node.children.find((c) => c.name === path[i]);
    if (!next) return null;
    node = next;
  }
  return node;
}

export function layoutFlame(
  root: FlameNode,
  width: number,
  rowHeight = FLAME_ROW_HEIGHT,
): { rects: FlameRect[]; height: number; total: number; depth: number } {
  if (root.value <= 0 || width <= 0) {
    return { rects: [], height: rowHeight, total: 0, depth: 0 };
  }

  const maxDepth = (node: FlameNode): number => {
    if (node.children.length === 0) return 0;
    let best = 0;
    for (const child of node.children) {
      const d = maxDepth(child);
      if (d > best) best = d;
    }
    return 1 + best;
  };

  const depth = maxDepth(root);
  const height = (depth + 1) * rowHeight;
  const rects: FlameRect[] = [];

  const walk = (
    node: FlameNode,
    x: number,
    w: number,
    d: number,
    path: string[],
  ) => {
    if (w < MIN_RECT_WIDTH) return;
    const nextPath = [...path, node.name];
    const y = height - (d + 1) * rowHeight;
    rects.push({
      name: node.name,
      value: node.value,
      x,
      y,
      width: w,
      height: rowHeight,
      depth: d,
      path: nextPath,
    });
    let cx = x;
    for (const child of node.children) {
      const cw = (child.value / node.value) * w;
      walk(child, cx, cw, d + 1, nextPath);
      cx += cw;
    }
  };

  walk(root, 0, width, 0, []);
  return { rects, height, total: root.value, depth };
}

export function shortFrameLabel(name: string): string {
  if (name === "all") return name;
  const noParen = name.split("(")[0] ?? name;
  const parts = noParen.split(".");
  if (parts.length >= 2) {
    return `${parts[parts.length - 2]}.${parts[parts.length - 1]}`;
  }
  return noParen;
}

function hash32(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function flameFill(
  name: string,
  stateColors: Record<string, string> = STATE_COLORS,
): string {
  if (name === "all") return "#3d3d3f";
  const state = stateColors[name];
  if (state) return state;
  const h = hash32(name);
  const hue = 18 + (h % 24);
  const sat = 38 + (h % 18);
  const lit = 50 + (h % 14);
  return `hsl(${hue} ${sat}% ${lit}%)`;
}

export function labelFill(fill: string): string {
  if (fill.startsWith("hsl")) {
    const m = fill.match(/(\d+(?:\.\d+)?)%\s*\)\s*$/);
    const lit = m ? Number(m[1]) : 50;
    return lit > 62 ? "#1d1d1f" : "#ffffff";
  }
  if (fill.startsWith("#") && (fill.length === 7 || fill.length === 4)) {
    const hex =
      fill.length === 4
        ? `#${fill[1]}${fill[1]}${fill[2]}${fill[2]}${fill[3]}${fill[3]}`
        : fill;
    const n = parseInt(hex.slice(1), 16);
    const r = (n >> 16) & 255;
    const g = (n >> 8) & 255;
    const b = n & 255;
    const lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
    return lum > 0.62 ? "#1d1d1f" : "#ffffff";
  }
  return "#ffffff";
}

export function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

/** Self-contained SVG for HTML export (hover via <title>). */
export function renderFlameSvg(
  root: FlameNode,
  width: number,
  stateColors: Record<string, string> = STATE_COLORS,
): string {
  const { rects, height, total } = layoutFlame(root, width);
  if (total === 0 || rects.length === 0) {
    return "";
  }
  const parts = [
    `<svg class="flame-svg" data-testid="flamegraph-svg" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" role="img">`,
  ];
  for (const rect of rects) {
    const fill = flameFill(rect.name, stateColors);
    const x = rect.x + 0.4;
    const y = rect.y + 0.4;
    const w = Math.max(0, rect.width - 0.8);
    const h = Math.max(0, rect.height - 0.8);
    const pct = total > 0 ? ((rect.value / total) * 100).toFixed(1) : "0";
    const title = `${escapeXml(rect.name)} · ${rect.value} (${pct}%)`;
    parts.push(
      `<g><title>${title}</title><rect x="${x.toFixed(2)}" y="${y.toFixed(2)}" width="${w.toFixed(2)}" height="${h.toFixed(2)}" rx="1.5" fill="${escapeXml(fill)}"/>`,
    );
    if (rect.width >= LABEL_MIN_WIDTH) {
      const textX = rect.x + 4;
      const textY = rect.y + rect.height / 2 + 3.5;
      const maxChars = Math.max(4, Math.floor((rect.width - 8) / 6.2));
      let label = shortFrameLabel(rect.name);
      if (label.length > maxChars) label = `${label.slice(0, maxChars - 1)}…`;
      parts.push(
        `<text x="${textX.toFixed(2)}" y="${textY.toFixed(2)}" fill="${labelFill(fill)}" font-size="10">${escapeXml(label)}</text>`,
      );
    }
    parts.push("</g>");
  }
  parts.push("</svg>");
  return parts.join("");
}
