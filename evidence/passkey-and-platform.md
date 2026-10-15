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
- No `CertificatePinner` usage was found in the app's own packages.

## Web app: a legacy Virgil E3Kit path

`my.cloaked.com` loads `/encryption/browser.asmjs.umd.js`, which is Virgil Security's E3Kit (`window.E3kit.EThree`, about 3.5 MB of asm.js). It also loads the Go module (`window.CloakedEncryption`). A Vuex getter, `authentication/isV2Encryption`, picks between them:

- **V2:** `CloakedEncryption.build()`, the Go library described in this report.
- **V1:** `EThree.initialize(jwt)`. It then calls `restorePrivateKey(secret_key)` or `backupPrivateKey(secret_key)`, which use Virgil Keyknox (cloud key storage). On `WrongKeyknoxPasswordError` during backup it calls `resetPrivateKeyBackup()` and backs up again.

V1 starts only when `state.encryption.secret_key` is non-empty. The store has a `setSecretKey` mutation, but nothing in the main bundle or any of its 29 lazily loaded chunks ever commits it. **In the build served on 2026-10-15, the V1 path appears unreachable, while its library is still downloaded on every page load.** Neither the extension nor the Android app contains E3Kit.

## Server-side storage seen from the clients

These web API calls show what the server holds for each account:
- `GET /api/v1/encryption/private-key/password/`: the private key wrapped by the password-derived secret box key.
- `GET /api/v1/encryption/recovery-code/`: the recovery code sealed to the user's public key.
- Password change sends `{ new_password: <auth key>, encrypted_private_key: <wrapped private key> }`.

The server therefore stores both the auth key (or a hash of it) **and** a ciphertext that the cheaper secret box key can open. That second item is the basis of the "cheapest verification path" point in the report.
