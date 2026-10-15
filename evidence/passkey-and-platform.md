# Passkey unlock, process isolation and the legacy web path

## Passkey unlock (WebAuthn PRF), Android 2.90.9

The classes are in `app.android.cloaked.encryption.v2.passkey` and `EncryptionUnlockOptionsRepository` (decompiled with JADX 1.5.6). One coroutine class was missing from the full JADX run and was recovered with `jadx --single-class 'app.android.cloaked.encryption.v2.EncryptionUnlockOptionsRepository$decryptEncryptionKeysWithPasskeyPrf$2'`.

1. **Unlock options come from the server** (`PasskeyUnlockOptions`):
   - `prfSalt`, one per account;
   - `publicKey` and `accountSalt`;
   - one `PasskeyUnlockOption(id, encryptedPrivateKey, credentialId)` per registered passkey.
2. **The app builds a WebAuthn assertion request** (`CredentialManagerPasskeyAssertionClient`):
   - `rpId: "cloaked.com"`, `userVerification: "required"`, timeout 1,800,000 ms;
   - `extensions.prf.eval.first = prfSalt`;
   - `challenge` = 32 bytes from `SecureRandom` **generated on the device**;
   - no `allowCredentials`, so any discoverable passkey for cloaked.com may answer.
3. **It reads `clientExtensionResults.prf.results.first`** (32 bytes, base64url).
4. **It derives a key** with `generatePasswordSecretBoxKey(prfOutput, accountSalt)`: the PRF output goes through the same Argon2id path as a password (64 MiB, t=1, p=4).
5. **It tries `decryptPrivateKey(key, encryptedPrivateKey)`** against each registered passkey's wrapped key until one decrypts.

The web app uses the same design:
- **Registration:** `getRegistrationOptions` returns `extensions.prf.eval.first`. The app calls `navigator.credentials.create`, checks `prf.enabled`, then does a `navigator.credentials.get` to obtain the PRF output.
- **Unlock:** `getEncryptionUnlockOptions` → `navigator.credentials.get` with `challenge: crypto.getRandomValues(...)` (generated on the client), `userVerification`, and the PRF eval. Unlike Android, it passes `allowCredentials` whenever the server supplies it.
- **"Confirm it's you" dialog:** the passkey option runs the same kind of assertion with a browser-generated challenge, never sends the result to the server, and then emits `verified`. It's an interface-level check. The password option in the same dialog is checked by the server (`POST /api/v1/user/{id}/password/confirm/` with the auth key).

## Encryption runs in its own Android process

From `AndroidManifest.xml`:

```xml
<service
    android:name="app.android.cloaked.encryption.v2.EncryptionProcessService"
    android:exported="false"
    android:process=":crypto"/>
```

The app reaches every encryption-library call over AIDL (`IEncryptionService`). It's the same Linux UID as the app (not `isolatedProcess`).

Other manifest and network details:
- `android:allowBackup="false"`.
- `network_security_config` trusts user-installed CAs only in debug builds.
- No `CertificatePinner` usage and no pin set were found. The app does install its own `CTTrustManager` (`datasource/network/conscrypt/CTTrustManager`, wired into OkHttp in `di/modules/NetworkModuleKt`). It requires embedded SCTs on the leaf certificate but does not verify the SCT signatures.

## Web app: a legacy Virgil E3Kit path

`my.cloaked.com` loads `/encryption/browser.asmjs.umd.js`, which is Virgil Security's E3Kit (`window.E3kit.EThree`, about 3.5 MB of asm.js). It also loads the Go module (`window.CloakedEncryption`). A Vuex getter, `authentication/isV2Encryption`, picks between them:

- **V2:** `CloakedEncryption.build()`, the Go library described in this report.
- **V1:** `EThree.initialize(jwt)`. It then calls `restorePrivateKey(secret_key)` or `backupPrivateKey(secret_key)`, which use Virgil Keyknox (cloud key storage). On `WrongKeyknoxPasswordError` during backup it calls `resetPrivateKeyBackup()` and backs up again.

V1 starts only when `state.encryption.secret_key` is non-empty. The store has a `setSecretKey` mutation, but nothing in the main bundle or any of its 29 lazily loaded chunks commits it. The store is persisted to `localStorage` under the key `vuex` (`vuex-persistedstate`, with the `encryption` module included), so a browser that ran an older build can still carry `secret_key` into the current one. The UI also has V1-specific elements (`is-v1-user`, and a recovery-kit PDF built from `secret_key`). **Current code never sets the V1 key, yet the path stays reachable through persisted state and the library downloads on every page load.** Neither the extension nor the Android app contains E3Kit.

## Server-side storage seen from the clients

These web API calls show what the server holds for each account:
- `GET /api/v1/encryption/private-key/password/`: the private key wrapped by the password-derived secret box key.
- `GET /api/v1/encryption/recovery-code/`: the recovery code sealed to the user's public key.
- Password change sends `{ new_password: <auth key>, encrypted_private_key: <wrapped private key> }`.

The server therefore stores both the auth key (or a hash of it) **and** a ciphertext that the cheaper secret box key can open. That second item is the basis of the "cheapest verification path" point in the report.
