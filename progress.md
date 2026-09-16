# Session Progress Log

## Current State

**Last Updated:** 2026-09-16
**Active Feature:** feat-066 import dump from clipboard (web + CLI)

## Status

### What's Done

- [x] Web: Paste dump 按钮、Ctrl/Cmd+V、粘贴文件、Clipboard API 失败时的粘贴模态框
- [x] CLI: `-c`/`--clipboard` 增加 Windows PowerShell `Get-Clipboard -Raw`
- [x] 真实子进程测试（假 `pbpaste` 脚本，不 mock 解析器）
- [x] 8 语言 i18n + README/help 文案

### What's In Progress

- [ ] `./init.sh` 全量验证与浏览器走查

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- 浏览器 Clipboard API 在非安全上下文 / 未授权时会失败，已用粘贴模态框与 `paste` 事件兜底。
- CLI 剪贴板依赖本机工具；无 `pbpaste`/`wl-paste`/`xclip`/`xsel`/PowerShell 时 `-c` 会报错。
- Very deep stacks are capped at 40 frames from the root (oldest side) so SVG height stays bounded.

## Decisions Made

- Stayed on `main` per AGENTS.md and the user request.
- One primary CTA remains **Choose dump(s)**; paste is a secondary import path.
- CLI 仍要求显式 `-c`（不与文件参数混用），与 feat-056 一致。

## Evidence of Completion

(pending `./init.sh`)
