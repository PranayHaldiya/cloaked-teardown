# Artifacts studied

All three clients were downloaded on 2026-10-15 and studied locally. No Cloaked account was created, no API was called beyond downloading the public files, and nothing was installed on a device.

| Client | Version | File | SHA-256 |
|---|---|---|---|
| Chrome extension | 3.9.0 (Manifest V3) | `encryption.wasm` | `047ae71868ff5869b42972093c339c74a341293a896d8744bdd11844d858d5fd` |
| Web app (my.cloaked.com) | live on 2026-10-15 | `encryption.wasm` | `e784a36db97a3a0304cb54186103907d310a82fc1443ec31c79c34252b9d8770` |
| Android | 2.90.9 (version code 210, min SDK 29, target SDK 36) | `app.android.cloaked.apk` (base split) | `d10525d3f218d404b794cabac42e535cc9b1c40083ecb4c87169026b2d879a8d` |
| Android, native crypto library | as above | `lib/x86_64/libgojnx.so` | `553fc3b0ffcda8ed9b0887bc3e5608858e5703be38158cac70aee8556ef06fbe` |

## Authenticity

- **Android:** the base APK is signed (APK Signature Scheme v2 and v3) by a certificate with SHA-256 `58:1D:49:7A:77:F9:97:EC:3D:5F:28:60:F8:76:0F:E2:75:C0:38:60:33:F4:8C:38:14:40:8B:6B:5E:3E:63:C3`. That fingerprint is listed for `app.android.cloaked` in Cloaked's own `https://www.cloaked.com/.well-known/assetlinks.json`, so a mirror can't have re-signed the file. To check it yourself, run `scripts/verify-android.sh`.
- **Extension:** downloaded directly from Google's Chrome Web Store update endpoint.
- **Web:** downloaded directly from `my.cloaked.com`.

## Build provenance of the encryption library

| Client | Toolchain | Notes |
|---|---|---|
| Extension | go1.21.0 (Aug 2023), `GOOS=js GOARCH=wasm` | Loaded through Go's `wasm_exec` shim; the embedded source paths point to a GitHub Actions runner (`/home/runner/work/encryption-sdk/…`) |
| Web | go1.21.0, `GOOS=js GOARCH=wasm` | Different build from the extension (also built on CI), but the same exported API and identical outputs |
| Android | go1.25.2, gomobile (`gobind`), golang.org/x/crypto v0.42.0, x/mobile 2025-09-11 | Built without `-trimpath`. The build info contains a local home-directory path, which suggests a developer machine rather than CI. It's omitted here. |

The Go module is `github.com/cloakedinc/encryption-sdk` (private). The Android bindings name it `mando_mobile`.
