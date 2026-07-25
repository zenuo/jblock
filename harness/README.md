# Harness artifacts

Agent-facing state for jblock lives at the repo root (`AGENTS.md`, `feature_list.json`,
`progress.md`, `init.sh`). This directory stores **generated verification evidence**.

## `e2e-results.json`

Produced by:

```bash
node scripts/e2e-features.mjs
# or (after web gates already ran in ./init.sh):
node scripts/e2e-features.mjs --skip-web
```

For every entry in `feature_list.json` the runner records:

- mapped `cargo test` names (must be listed and pass)
- static checks (fixtures, PatternKind/Scenario wiring, UI symbols, CI, locales, …)
- optional web gates (`wasm` / `lint` / `typecheck` / `build`) unless `--skip-web`

Pattern features also require `e2e_all_pattern_fixtures_detect_expected_kinds`, which
feeds each fixture through the public `analyze` / `analyze_series` API.

`./init.sh` runs the matrix with `--skip-web` after the normal cargo + web gates.
