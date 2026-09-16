# Session Progress Log

## Current State

**Last Updated:** 2026-09-16
**Active Feature:** feat-067 thread-state filter does not carry to a new analysis

## Status

### What's Done

- [x] Results 状态筛选默认 ALL，不再在有 BLOCKED 时自动筛 BLOCKED
- [x] 新 dump / 换分析时重置筛选；Results 按 dump digest remount
- [x] 点击 Thread states 仍只筛选当前这次分析

### What's In Progress

- [ ] (none)

### What's Next

1. Optional: click a held-lock id in the threads table to jump to that lock's contention group
2. Optional: show reentrancy count (`×2`) next to a unique lock

### Unresolved Risks

- 浏览器 Clipboard API 在非安全上下文 / 未授权时会失败，已用粘贴模态框与 `paste` 事件兜底。
- CLI 剪贴板依赖本机工具；无 `pbpaste`/`wl-paste`/`xclip`/`xsel`/PowerShell 时 `-c` 会报错。

## Decisions Made

- Stayed on `main` per AGENTS.md.
- feat-015 “default emphasis on BLOCKED” 视为会串到新分析的 bug，改为每次分析从 All 开始。

## Evidence of Completion

```text
$ pnpm -C web run lint && pnpm -C web run typecheck
$ node scripts/e2e-features.mjs --skip-web
Summary: 67/67 features PASS
```

Browser: 当前页点 WAITING → Threads (1/17) 且 State=WAITING；新标签 Load sample → State ALL、Threads (12/17)，BLOCKED 未自动选中。
