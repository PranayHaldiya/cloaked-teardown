#!/usr/bin/env bash
# Check that the downloaded APK is signed by a certificate Cloaked itself publishes
# in https://www.cloaked.com/.well-known/assetlinks.json (Android App Links).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
apk="$root/artifacts/android/xapk/app.android.cloaked.apk"

# The base APK is signed with APK Signature Scheme v2/v3 only, so keytool cannot read it.
cert=$(uvx -q --from androguard androguard sign --hash sha256 "$apk" | awk '/sha256/ {print $2}' | tr 'a-f' 'A-F' | sed 's/../&:/g; s/:$//')
echo "APK signing certificate SHA-256: $cert"

published=$(curl -fsSL https://www.cloaked.com/.well-known/assetlinks.json |
  jq -r '.[] | select(.target.package_name == "app.android.cloaked") | .target.sha256_cert_fingerprints[]' | sort -u)

if grep -qx "$cert" <<<"$published"; then
  echo "OK: certificate is listed in cloaked.com/.well-known/assetlinks.json"
else
  echo "FAIL: certificate is not published by cloaked.com; do not trust this APK" >&2
  exit 1
fi
