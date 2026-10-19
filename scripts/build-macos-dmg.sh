#!/usr/bin/env bash
# 在 Apple Silicon Mac 上本地打 v2.0.4 DMG（勿用 GitHub Actions）。
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "此脚本只能在 macOS 上运行。" >&2
  exit 1
fi

npm ci
npm run test:gateway
cargo test --manifest-path src-tauri/Cargo.toml window_position::tests --lib
npm run tauri:build -- --bundles dmg

DMG=(src-tauri/target/release/bundle/dmg/*.dmg)
if [[ ${#DMG[@]} -ne 1 || ! -f "${DMG[0]}" ]]; then
  echo "未找到 DMG: src-tauri/target/release/bundle/dmg/" >&2
  exit 1
fi

OUT_DIR="release-artifacts"
mkdir -p "$OUT_DIR"
cp -f "${DMG[0]}" "$OUT_DIR/"
echo "OK: $OUT_DIR/$(basename "${DMG[0]}")"
