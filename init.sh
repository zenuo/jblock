#!/bin/bash
set -e

echo "=== Harness Initialization ==="

echo "=== cargo test ==="
cargo test

echo "=== pnpm -C web install ==="
pnpm -C web install

echo "=== pnpm -C web run wasm (generates web/src/wasm before typecheck/lint) ==="
pnpm -C web run wasm

echo "=== pnpm -C web run lint ==="
pnpm -C web run lint

echo "=== pnpm -C web run typecheck ==="
pnpm -C web run typecheck

echo "=== pnpm -C web run build ==="
pnpm -C web run build

echo "=== Verification Complete ==="
echo ""
echo "Next steps:"
echo "1. Read feature_list.json to see current feature state"
echo "2. Pick ONE unfinished feature to work on"
echo "3. Implement only that feature"
echo "4. Re-run verification before claiming done"
