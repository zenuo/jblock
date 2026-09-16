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
- [x] `./init.sh` 全量验证与浏览器走查

### What's In Progress

- [ ] (none)

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

```text
$ ./init.sh
cargo test --features cli  118 passed
pnpm -C web run lint/typecheck/build  ok
node scripts/e2e-features.mjs --skip-web
Summary: 66/66 features PASS
```

CLI（假 `pbpaste` 输出 deadlock fixture）:

```text
$ PATH=/tmp/jblock-fake-clip:$PATH cargo run --quiet --features cli --bin jblock -- -c -s findings
jblock · jstack · 20 threads
source: clipboard
FINDINGS
  x DEADLOCK (critical)
      deadlock-0 → deadlock-1 → deadlock-2 → deadlock-0
exit 1
```

Browser: Paste dump 授权后立即分析；首页 Ctrl+V 同样导入。工作区文件名 `clipboard.txt`，Findings 显示 3 线程死锁。
