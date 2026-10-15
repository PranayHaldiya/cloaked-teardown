#!/usr/bin/env bash
# Print SHA-256 and Go build info for the artifacts studied, to compare with evidence/artifacts.md.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
a="$root/artifacts"

sha256sum \
  "$a/extension/src/encryption.wasm" \
  "$a/web/encryption.wasm" \
  "$a/android/xapk/app.android.cloaked.apk" | sed "s|$a/||"

lib="$a/android/lib/x86_64/libgojnx.so"
mkdir -p "$(dirname "$lib")"
unzip -q -o -j "$a/android/xapk/app.android.cloaked.apk" 'lib/x86_64/libgojnx.so' -d "$(dirname "$lib")"
sha256sum "$lib" | sed "s|$a/||"

# Go build info: compiler and module versions (developer paths are printed too; do not publish them).
go version -m "$lib" | grep -E 'go1|golang.org/x/(crypto|mobile)' || true
