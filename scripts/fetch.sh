#!/usr/bin/env bash
# Download the three public Cloaked clients into ./artifacts for local, static study.
# Nothing here logs in, calls Cloaked's APIs, or installs anything on a device.
#
# Requires: curl, unzip, apkeep (cargo install apkeep), keytool or androguard for cert checks.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/artifacts"
mkdir -p "$out"/{extension,android,web}

echo "==> Chrome extension (oppdbdefikkkihgbliidckokhgcmmiga) from Google's CRX update endpoint"
curl -fsSL -o "$out/extension/cloaked.crx" \
  "https://clients2.google.com/service/update2/crx?response=redirect&prodversion=141.0&acceptformat=crx2,crx3&x=id%3Doppdbdefikkkihgbliidckokhgcmmiga%26uc"
# A CRX is a zip with a header; unzip skips the header with a warning.
unzip -q -o "$out/extension/cloaked.crx" -d "$out/extension/src" 2>/dev/null || true
echo "    version: $(grep -o '"version": *"[^"]*"' "$out/extension/src/manifest.json")"

echo "==> Android app (app.android.cloaked)"
# APKPure mirrors Play Store builds. It has shipped tampered apps before, so
# scripts/verify-android.sh checks the signing certificate against Cloaked's own
# assetlinks.json before anything else trusts the file.
apkeep -d apk-pure -o 'acknowledge_dangers=true' -a app.android.cloaked "$out/android"
unzip -q -o "$out/android/app.android.cloaked.xapk" -d "$out/android/xapk"

echo "==> Web app encryption module (my.cloaked.com)"
for path in encryption.js encryption.wasm; do
  curl -fsSL -A 'Mozilla/5.0' -o "$out/web/$path" "https://my.cloaked.com/$path"
done

echo "==> Done. Next: scripts/verify-android.sh, then scripts/hashes.sh"
