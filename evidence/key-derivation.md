# Key derivation: decompilation evidence

Source of truth: Android `libgojnx.so` for x86_64 (SHA-256 `553fc3b0…6fbe`), analyzed with Ghidra 12.1.2 through REA 6.5.0. Go kept its symbol names, so the functions below are named exactly as in Cloaked's source. Line numbers come from the Go function table.

**About the addresses:** they are Ghidra program addresses, and Ghidra loads this library at image base `0x100000`. Subtract `0x100000` to get ELF virtual addresses for `objdump` or `readelf` (for example, `0x216f60` becomes `0x116f60`).

The parameters were then confirmed **at runtime** in the web and extension builds (see `kdf-vectors.json`), and reproduced byte for byte by an independent implementation (`kdf-cost verify`).

## The Argon2id wrapper

`utilities.makeArgon2IDHash(password, salt []byte, memory, time uint32, threads uint8, keyLen uint32)` at `0x216f60` (`utilities/privateutils.go:142`).

It calls `golang.org/x/crypto/argon2.deriveKey` with mode `2` (Argon2id). How the register moves map arguments (Go register ABI):

```text
0x216fd1: MOV byte ptr [RSP + 0x30], R11B   ; threads  (stack arg of deriveKey)
0x216fd6: MOV EDX, dword ptr [RSP + 0x238]  ; keyLen   (caller's stack arg)
0x216fdd: MOV dword ptr [RSP + 0x34], EDX
0x216fe1: MOV R11D, R9D                     ; memory   <- caller R9
                                            ; time     <- caller R10 (left in place)
0x216ff6: MOV EAX, 0x2                      ; mode = argon2id
0x217000: CALL golang.org/x/crypto/argon2.deriveKey
```

It then formats the result as `argon2id$v=19$m=<memory>$t=<time>$p=<threads>$T=<keyLen>$<salt>$<key>`.

## Call sites

| Function (address, source) | Input | Salt | memory | time | threads | keyLen |
|---|---|---|---|---|---|---|
| `GenerateValidPasswordSecretBoxKey` (`0x218460`, `publicutils.go:64`) | password | account salt | 65536 KiB | **1** | 4 | 32 |
| `GenerateValidPasswordAuthKey` (`0x218540`, `publicutils.go:82`) | secret box key (must be 32 bytes) | account salt | 65536 KiB | **5** | 4 | 32 |
| `GenerateValidUsernameHash` (`0x2182e0`, `publicutils.go:34`), step 1 | username | fixed 32-byte constant | 65536 KiB | 1 | 4 | 32 |
| `GenerateValidUsernameHash`, step 2 | username | output of step 1 | 65536 KiB | 1 | 4 | 32 |
| `GenerateValidRecoveryCode` (`0x218660`, `publicutils.go:109`), step 1 | recovery code | account salt | 65536 KiB | 1 | 4 | 32 |
| `GenerateValidRecoveryCode`, step 2 (validation hash) | recovery code | output of step 1 | 65536 KiB | 1 | 4 | 32 |

The auth-key call site, for example (Ghidra's decompiler dropped these arguments; the assembly shows them):

```text
0x2185be: MOV dword ptr [RSP], 0x20         ; keyLen  = 32
0x2185d4: MOV R9D, 0x10000                  ; memory  = 65536 KiB
0x2185da: MOV R10D, 0x5                     ; time    = 5
0x2185e0: MOV R11D, 0x4                     ; threads = 4
0x2185e6: CALL utilities.makeArgon2IDHash
```

## Random values

- `GenerateValidAccountSalt` (`0x218260`): 32 bytes from `crypto/rand`, base64url.
- `GenerateValidRecoveryCode`: 32 bytes from `crypto/rand`. The code is then sealed to the user's own public key with `boxSealAnonymous` (NaCl sealed box), so the app can show it again after you log in.

## Username hash constant

`GenerateValidUsernameHash` writes a 32-byte ASCII constant into a buffer and uses it as the Argon2id salt for every user. The constant is in every shipped client. This write-up describes the construction but doesn't reproduce the constant or provide a ready-made hashing tool.
