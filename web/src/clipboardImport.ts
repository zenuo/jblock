/** Display name used when the dump came from the clipboard rather than a file. */
export const CLIPBOARD_DUMP_NAME = "clipboard.txt";

const EDITABLE_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

export type ClipboardReadResult =
  | { ok: true; text: string }
  | { ok: false; reason: "unavailable" | "denied" | "empty" };

export type ClipboardPermissionState =
  | "granted"
  | "denied"
  | "prompt"
  | "unknown";

/** Strip a UTF-8 BOM so pasted dumps match file reads. */
export function stripBom(text: string): string {
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

export function clipboardTextIsEmpty(text: string): boolean {
  return stripBom(text).trim().length === 0;
}

/**
 * True when a paste should go to the focused field instead of becoming a dump.
 * Accepts a duck-typed element so Node tests do not need jsdom.
 */
export function isEditablePasteTarget(
  el:
    | {
        tagName?: string;
        isContentEditable?: boolean;
        closest?: (selector: string) => unknown;
      }
    | null
    | undefined,
): boolean {
  if (!el) return false;
  const tag = (el.tagName || "").toUpperCase();
  if (EDITABLE_TAGS.has(tag)) return true;
  if (el.isContentEditable) return true;
  if (typeof el.closest === "function") {
    return Boolean(el.closest("input, textarea, select, [contenteditable=true]"));
  }
  return false;
}

export function textFromClipboardData(
  getData: ((type: string) => string) | null | undefined,
): string {
  if (!getData) return "";
  try {
    return stripBom(getData("text/plain") || "");
  } catch {
    return "";
  }
}

/**
 * `clipboard-read` permission without triggering Chrome's native Paste chip.
 * Querying is silent; only `readText()` while state is `prompt` shows that UI.
 */
export async function queryClipboardReadState(
  permissions: Pick<Permissions, "query"> | null | undefined,
): Promise<ClipboardPermissionState> {
  if (!permissions || typeof permissions.query !== "function") return "unknown";
  try {
    const status = await permissions.query({
      name: "clipboard-read" as PermissionName,
    });
    if (
      status.state === "granted" ||
      status.state === "denied" ||
      status.state === "prompt"
    ) {
      return status.state;
    }
    return "unknown";
  } catch {
    return "unknown";
  }
}

/** Only auto-read when the browser already allowed it (no native Paste prompt). */
export function shouldAutoReadClipboard(
  state: ClipboardPermissionState,
): boolean {
  return state === "granted";
}

/** Read dump text via the Async Clipboard API (requires a user gesture). */
export async function readSystemClipboard(
  clipboard: { readText: () => Promise<string> } | undefined | null,
): Promise<ClipboardReadResult> {
  if (!clipboard || typeof clipboard.readText !== "function") {
    return { ok: false, reason: "unavailable" };
  }
  try {
    const text = stripBom(await clipboard.readText());
    if (clipboardTextIsEmpty(text)) return { ok: false, reason: "empty" };
    return { ok: true, text };
  } catch {
    return { ok: false, reason: "denied" };
  }
}
