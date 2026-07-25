# Session Progress Log

## Current State

**Last Updated:** 2026-07-25 12:15
**Active Feature:** docs + header GitHub link (PR #23)

## Status

### What's Done

- [x] README 增加「本地部署」
- [x] Header 右上角 GitHub icon → https://github.com/zenuo/jblock（8 语言 aria-label）

### What's In Progress

- [ ] None

### What's Next

1. Optionally: dual CI artifact (`VITE_BASE=/`) + Release zip for zero-toolchain local deploy

## Decisions Made

- GitHub link sits after Help in `header-actions`, matches existing `icon-btn` style; opens in new tab.

## Evidence of Completion

- lint + typecheck pass; e2e **43/43 PASS**
