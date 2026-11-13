/**
 * Batch import helpers (feat-074): flatten picked files, dropped files and
 * dropped directories into text items, skipping binaries.
 *
 * Duck-typed so `scripts/test-import-files.mjs` can run them in Node.
 */

import { stripBom } from "./clipboardImport.ts";

/** Bytes inspected to decide whether a file is binary. */
export const BINARY_SNIFF_BYTES = 4096;

export interface ImportItem {
  name: string;
  text: string;
}

export interface ImportBatch {
  items: ImportItem[];
  /** Files skipped because they look binary (archives, images, class files…). */
  skipped: string[];
  /** Gzip files that could not be decompressed (no DecompressionStream, corrupt data). */
  gzipFailed: string[];
}

/** Minimal File surface used here. */
export interface FileLike {
  name: string;
  /** Path inside a picked folder (`webkitdirectory`), e.g. `logs/gc.log.0`. */
  webkitRelativePath?: string;
  slice(start?: number, end?: number): { arrayBuffer(): Promise<ArrayBuffer> };
  text(): Promise<string>;
  /** Present on real Files; needed to gunzip `.gz` logs (feat-082). */
  stream?: () => ReadableStream<Uint8Array>;
}

/** Minimal FileSystemEntry surface (drag-and-drop directories). */
export interface EntryLike {
  isFile: boolean;
  isDirectory: boolean;
  name: string;
  fullPath?: string;
  file?: (ok: (f: FileLike) => void, err?: (e: unknown) => void) => void;
  createReader?: () => {
    readEntries: (ok: (entries: EntryLike[]) => void, err?: (e: unknown) => void) => void;
  };
}

/** Gzip magic bytes (rotated GC logs are often `gc.log.3.gz`). */
export function isGzip(bytes: Uint8Array): boolean {
  return bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b;
}

/** Gunzip a file with the platform `DecompressionStream`; null when unavailable or corrupt. */
async function gunzipText(file: FileLike): Promise<string | null> {
  if (typeof DecompressionStream === "undefined" || !file.stream) return null;
  try {
    // lib.dom types DecompressionStream as BufferSource-in; file streams yield Uint8Array chunks.
    const gunzip = new DecompressionStream("gzip") as unknown as ReadableWritablePair<Uint8Array, Uint8Array>;
    const stream = file.stream().pipeThrough(gunzip);
    return await new Response(stream).text();
  } catch {
    return null;
  }
}

/** A NUL byte in the first bytes means binary; text logs never contain one. */
export function looksBinary(bytes: Uint8Array): boolean {
  const n = Math.min(bytes.length, BINARY_SNIFF_BYTES);
  for (let i = 0; i < n; i++) {
    if (bytes[i] === 0) return true;
  }
  return false;
}

/** Hidden files and OS metadata are never logs. */
export function isIgnoredName(name: string): boolean {
  const base = name.split("/").pop() ?? name;
  return base.startsWith(".") || base === "Thumbs.db" || base === "desktop.ini";
}

/** Display name: the folder-relative path when present, else the file name. */
export function displayName(file: FileLike): string {
  return file.webkitRelativePath || file.name;
}

/** Natural sort so `gc.log.2` < `gc.log.10` and folders stay together. */
export function compareNames(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
}

function fileFromEntry(entry: EntryLike): Promise<FileLike> {
  return new Promise((resolve, reject) => entry.file!(resolve, reject));
}

function readAllEntries(entry: EntryLike): Promise<EntryLike[]> {
  const reader = entry.createReader!();
  const out: EntryLike[] = [];
  // readEntries returns batches (Chrome: 100 at a time) until an empty batch.
  return new Promise((resolve, reject) => {
    const next = () =>
      reader.readEntries((batch) => {
        if (batch.length === 0) {
          resolve(out);
          return;
        }
        out.push(...batch);
        next();
      }, reject);
    next();
  });
}

/** Recursively collect files below dropped entries, tagging each with its path. */
export async function filesFromEntries(entries: EntryLike[]): Promise<FileLike[]> {
  const files: FileLike[] = [];
  const walk = async (entry: EntryLike): Promise<void> => {
    if (isIgnoredName(entry.name)) return;
    if (entry.isFile && entry.file) {
      const f = await fileFromEntry(entry);
      const path = (entry.fullPath ?? entry.name).replace(/^\//, "");
      files.push(path === f.name ? f : wrap(f, path));
    } else if (entry.isDirectory && entry.createReader) {
      for (const child of await readAllEntries(entry)) await walk(child);
    }
  };
  for (const e of entries) await walk(e);
  return files;
}

/** Keep the File's methods but expose the dropped path as `webkitRelativePath`. */
function wrap(f: FileLike, path: string): FileLike {
  return {
    name: f.name,
    webkitRelativePath: path,
    slice: (start, end) => f.slice(start, end),
    text: () => f.text(),
  };
}

/** Read files as text, skipping binaries and ignored names; sorted by display name. */
export async function readImportBatch(files: FileLike[]): Promise<ImportBatch> {
  const items: ImportItem[] = [];
  const skipped: string[] = [];
  const gzipFailed: string[] = [];
  const sorted = [...files].sort((a, b) => compareNames(displayName(a), displayName(b)));
  for (const file of sorted) {
    const name = displayName(file);
    if (isIgnoredName(name)) continue;
    const head = new Uint8Array(await file.slice(0, BINARY_SNIFF_BYTES).arrayBuffer());
    if (isGzip(head)) {
      const text = await gunzipText(file);
      if (text == null) gzipFailed.push(name);
      else items.push({ name: name.replace(/\.gz$/i, ""), text: stripBom(text) });
      continue;
    }
    if (looksBinary(head)) {
      skipped.push(name);
      continue;
    }
    items.push({ name, text: stripBom(await file.text()) });
  }
  return { items, skipped, gzipFailed };
}

/**
 * Append newly imported items to an existing batch. An item with the same name
 * and text is not added twice; same name with new text gets a ` (2)` suffix.
 */
export function appendItems(existing: ImportItem[], added: ImportItem[]): ImportItem[] {
  const out = [...existing];
  for (const item of added) {
    if (out.some((x) => x.name === item.name && x.text === item.text)) continue;
    let name = item.name;
    for (let n = 2; out.some((x) => x.name === name); n++) name = `${item.name} (${n})`;
    out.push({ name, text: item.text });
  }
  return out;
}
