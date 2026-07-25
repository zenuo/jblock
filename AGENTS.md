# jblock

Single-page Java thread dump analyzer. Rust parser compiled to WebAssembly,
consumed by a React + Vite frontend. See `README.md` for the full command list
and architecture.

## Startup Workflow

Before writing code:

1. **Confirm working directory** with `pwd` (repo root).
2. **Read this file** completely.
3. **Read project docs**: `README.md` (architecture + commands).
4. **Run `./init.sh`** to verify the environment is healthy.
5. **Read `feature_list.json`** to see current feature state.
6. **Review recent commits** with `git log --oneline -5`.

If baseline verification is failing, repair that first before adding new scope.

## Branching Model (Trunk-Based)

Use **trunk-based development**. Do not open a long-lived branch per feature by default.

- **`main` is the only long-lived branch.** Keep it green (`./init.sh` / CI).
- **Short-lived branches only**: create a branch for a coherent chunk of work, merge via PR quickly, then delete the branch. Prefer hours–a day of work, not multi-week feature branches.
- **Batch related work**: one PR may cover several related `feature_list.json` items when they share the same theme and verification (e.g. a results-UX batch). Avoid one PR per tiny checklist row.
- **Direct small fixes**: trivial docs/typo/config fixes may land as a single short-lived PR with minimal scope; do not invent a parallel `develop` / release-branch flow.
- **No Git Flow**: do not introduce long-lived `develop`, release, or per-environment branches unless the human explicitly asks.
- **CI/CD**: GitHub Actions on PRs/pushes; Pages deploy from `main` only. After merge, stay on / pull latest `main` before the next task.

Cloud / Cursor agents still use the required short-lived branch name template for PRs; treat that as ephemeral integration, not a standing feature branch.

## Working Rules

- **Coherent scope**: pick one unfinished feature, or one tightly related batch, from `feature_list.json`.
- **Verification required**: don't claim done without running the verification commands below.
- **Update artifacts**: before ending a session, update `progress.md` and `feature_list.json`.
- **Stay in scope**: don't modify files unrelated to the current feature / batch.
- **Leave clean state**: the next session must be able to run `./init.sh` immediately.

## Required Artifacts

- `feature_list.json` — feature state tracker (source of truth).
- `progress.md` — session continuity log.
- `init.sh` — standard startup and verification path.
- `scripts/e2e-features.mjs` — walks every `feature_list.json` entry; writes `harness/e2e-results.json`.
- `session-handoff.md` — optional, for larger multi-session work.

## Definition of Done

A feature is done only when ALL of the following are true:

- [ ] Target behavior is implemented.
- [ ] Required verification actually ran (tests / lint / type-check / build).
- [ ] Evidence recorded in `feature_list.json` or `progress.md`.
- [ ] Repository remains restartable from the standard startup path (`./init.sh`).

## End of Session

Before ending a session:

1. Update `progress.md` with current state.
2. Update `feature_list.json` with new feature status.
3. Record any unresolved risks or blockers.
4. Commit with a descriptive message once work is in a safe state.
5. Leave the repo clean enough for the next session to run `./init.sh` immediately.

## Verification Commands

```bash
# Full verification (recommended)
./init.sh
```

Required checks:
- `cargo test`
- `pnpm -C web install`
- `pnpm -C web run wasm` (regenerates `web/src/wasm/`; must run before typecheck/lint)
- `pnpm -C web run lint`
- `pnpm -C web run typecheck`
- `pnpm -C web run build`
- `node scripts/e2e-features.mjs --skip-web` (per-feature matrix → `harness/e2e-results.json`; included in `./init.sh`)

Record command + output as Verification Evidence in `progress.md` / `session-handoff.md`.
Inspect `harness/e2e-results.json` for per-feature pass/fail detail.

## Skills

- `harness-creator` (`.cursor/skills/harness-creator/SKILL.md`): build, audit, and
  improve coding-agent harnesses (AGENTS.md, feature/state tracking, verification
  gates, session handoff). Use it when a coding agent is unreliable across
  sessions or when creating/assessing `AGENTS.md`, `feature_list.json`, `init.sh`,
  `progress.md`, or session-handoff files. Bundled scripts (run with `node`):
  `scripts/create-harness.mjs`, `scripts/validate-harness.mjs`,
  `scripts/render-assessment-html.mjs`, `scripts/run-benchmark.mjs`.
- `ui-design-brain` (`.cursor/skills/ui-design-brain/SKILL.md`): production UI
  patterns (60+ components). Prefer its design philosophies (e.g. Apple-level
  Minimal) when redesigning web surfaces.

## Cursor Cloud specific instructions

### Layout & services

- Rust WASM crate at repo root (`Cargo.toml`, `src/lib.rs`, `src/parser.rs`).
- Frontend lives in `web/` (React + Vite + TypeScript). All `pnpm` commands run
  from `web/`.
- There is a single dev service: the Vite dev server (`web`, port `5173`).

### Non-obvious gotchas

- The parsing logic is pure Rust in `src/parser.rs` (no `wasm-bindgen`) so it is
  unit-testable with `cargo test` on the host target; `src/lib.rs` only holds the
  wasm bindings. Add parser tests in `parser.rs`.
- `web/src/wasm/` is a **generated** wasm-pack artifact (gitignored). It must
  exist before `vite`/`tsc` can resolve imports. Run `pnpm run wasm` (or
  `pnpm run dev`, which builds it first) to (re)generate it.
- WASM is **not** hot-reloaded. After editing any `src/*.rs`, re-run
  `pnpm run wasm` (or restart `pnpm run dev`); Vite HMR only tracks the JS/TS.
- The WASM is imported via `wasm-pack --target web`; Vite resolves the
  `.wasm` asset through `new URL(..., import.meta.url)`, so no extra Vite wasm
  plugin is needed.
- pnpm blocks `esbuild`'s install script by default; it is allow-listed via
  `pnpm.onlyBuiltDependencies` in `web/package.json` (avoid the interactive
  `pnpm approve-builds`).
- Keep `web/src/types.ts` in sync with the Rust `Analysis` struct in
  `src/parser.rs` (serde serializes enums as kebab-case, e.g. `thread-mx-bean`).

### Verify quickly

- Parser: `cargo test`
- Frontend: from `web/` run `pnpm run typecheck`, `pnpm run lint`,
  `pnpm run build`, and `pnpm run dev` (then load the "Load sample" button).
