# Method: reproduce every finding

These steps were run on Arch Linux (x86_64) in October 2026. Everything is local and static, except one step that runs Cloaked's WebAssembly module in Node with fixed test inputs. That module makes no network calls.

## Tools

| Tool | Used for |
|---|---|
| [REA](https://github.com/morluto/rea) 6.5.0 (`npx rea-agents setup`) | Agent-driven reverse engineering over MCP; drives Ghidra and the JavaScript analysis |
| Ghidra 12.1.2 + OpenJDK | Decompiling the Android native Go library |
| JADX 1.5.6 | Decompiling the Android app's Kotlin/Java |
| apkeep (`cargo install apkeep`) | Downloading the Android app |
| androguard (`uvx --from androguard androguard`) | Reading APK v2/v3 signing certificates |
| Go 1.21+ | `go version -m`, to read Go build info |
| Node 22+ | Running the WASM module locally |
| Rust (stable) | `kdf-cost` |

## 1. Download and check the clients

```bash
./scripts/fetch.sh            # extension CRX, Android XAPK, web encryption.js + .wasm -> artifacts/
./scripts/verify-android.sh   # APK certificate must be listed in cloaked.com/.well-known/assetlinks.json
./scripts/hashes.sh           # compare with evidence/artifacts.md
```

The Chrome Web Store, the APK mirror and `my.cloaked.com` all serve the current version, so your hashes will differ once Cloaked ships an update. The findings still apply as long as the function names and parameters below are unchanged.

## 2. Identify the encryption library

```bash
strings -n 6 artifacts/extension/src/encryption.wasm | grep -o 'github.com/cloakedinc/encryption-sdk[A-Za-z0-9_./]*' | sort -u
go version -m artifacts/android/lib/x86_64/libgojnx.so
```

Go keeps full package and function names in its binaries. Both builds expose the same `utilities.*` functions (`GenerateValidPasswordSecretBoxKey`, `GenerateValidPasswordAuthKey`, `GenerateValidUsernameHash`, `makeArgon2IDHash`, and others).

## 3. Decompile the key derivation (REA + Ghidra)

With REA registered in your agent (Claude Code, Codex and others), open the Android library and decompile the call sites:

```text
open_binary         path=<abs>/artifacts/android/lib/x86_64/libgojnx.so  provider_id=ghidra
search_procedures   pattern=encryption-sdk
procedure_pseudo_code  procedure=0x218460   # GenerateValidPasswordSecretBoxKey
procedure_assembly     procedure=0x218540   # GenerateValidPasswordAuthKey (decompiler drops its args)
procedure_assembly     procedure=0x216f60   # makeArgon2IDHash: maps caller registers to deriveKey
```

The Go register ABI on amd64 passes integer arguments in `RAX, RBX, RCX, RDI, RSI, R8, R9, R10, R11`. `makeArgon2IDHash(password, salt []byte, memory, time uint32, threads uint8, keyLen uint32)` takes `memory` in R9, `time` in R10, `threads` in R11 and `keyLen` on the stack. [`evidence/key-derivation.md`](evidence/key-derivation.md) lists every call site.

Addresses are Ghidra program addresses (image base `0x100000`) for the x86_64 library with SHA-256 `553fc3b0…6fbe`. Subtract `0x100000` to use them with `objdump --start-address`. For another build, search by function name instead.

## 4. Confirm at runtime in the browser builds

```bash
node scripts/vectors.mjs artifacts/web/encryption.js artifacts/web/encryption.wasm
node scripts/vectors.mjs artifacts/web/encryption.js artifacts/extension/src/encryption.wasm
```

Both commands print the encoded hash strings, which include `m=65536$t=1$p=4$T=32` (secret box key) and `t=5` (auth key). Both outputs should equal [`evidence/kdf-vectors.json`](evidence/kdf-vectors.json).

## 5. Reproduce the keys independently

```bash
cd kdf-cost
cargo run --release -- verify ../evidence/kdf-vectors.json   # byte-for-byte match with Cloaked's output
cargo run --release -- bench                                 # cost per guess vs Bitwarden / RFC 9106 / OWASP
cargo test --release
```

`kdf-cost` uses the RustCrypto `argon2` crate and shares no code with Cloaked's implementation. A match proves the parameters and the order of derivations.

## 6. Android: passkey unlock and process isolation

```bash
jadx -d artifacts/android/jadx artifacts/android/xapk/app.android.cloaked.apk
# The full run skips a few classes; recover the PRF coroutine on its own:
jadx --single-class 'app.android.cloaked.encryption.v2.EncryptionUnlockOptionsRepository$decryptEncryptionKeysWithPasskeyPrf$2' \
     -d artifacts/android/single artifacts/android/xapk/app.android.cloaked.apk
```

Read these:
- `app/android/cloaked/encryption/v2/passkey/CredentialManagerPasskeyAssertionClient.java`: the WebAuthn request, the device-generated challenge and the PRF eval.
- `EncryptionServiceClient.java` (`j` = `generatePasswordSecretBoxKey`, `e` = `decryptPrivateKey`) and the recovered coroutine: how the PRF output is used.
- `resources/AndroidManifest.xml`: `EncryptionProcessService` with `android:process=":crypto"`.

## 7. Web app: the legacy E3Kit path

```bash
curl -s https://my.cloaked.com/ | grep -o 'src="[^"]*\.js"'
```

1. Download `index-*.js` and the `assets/*.js` chunks it references.
2. Search for `window.E3kit`, `isV2Encryption`, `restorePrivateKey`, `backupPrivateKey` and `setSecretKey`.
3. Check that no file commits `setSecretKey`.

## Ground rules followed

- Only publicly distributed client files were studied, on my own machine.
- No accounts, no login, no requests to Cloaked's APIs, and no attempts against their servers.
- Published material is limited to hashes, addresses, parameters and short excerpts for commentary. No Cloaked code, binaries or assets are redistributed.
- Developer-identifying build paths and the username-hash salt constant are left out.
