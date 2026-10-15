//! kdf-cost: reproduce Cloaked's client-side key derivation from published
//! test vectors, and compare its per-guess cost with other password managers.
//!
//! Usage:
//!   kdf-cost verify <vectors.json>
//!   kdf-cost bench [iterations]

use std::{env, fs, process::ExitCode, time::Instant};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};

const KIB_64MIB: u32 = 64 * 1024;

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    note: &'static str,
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

/// Cloaked's two Argon2id call shapes, observed in all three clients.
const VAULT_KEY: Profile = Profile {
    name: "cloaked vault key",
    note: "password -> secret box key (unwraps the private key)",
    memory_kib: KIB_64MIB,
    passes: 1,
    lanes: 4,
};
const LOGIN_KEY: Profile = Profile {
    name: "cloaked login key",
    note: "secret box key -> auth key (the password the server sees)",
    memory_kib: KIB_64MIB,
    passes: 5,
    lanes: 4,
};

const COMPARISONS: [Profile; 3] = [
    Profile {
        name: "bitwarden default",
        note: "Bitwarden help center, Oct 2026: 32 MiB, 6 iterations, 4 threads",
        memory_kib: 32 * 1024,
        passes: 6,
        lanes: 4,
    },
    Profile {
        name: "rfc 9106 (64 MiB)",
        note: "RFC 9106 second recommended option",
        memory_kib: KIB_64MIB,
        passes: 3,
        lanes: 4,
    },
    Profile {
        name: "owasp minimum",
        note: "OWASP Password Storage Cheat Sheet, m=46 MiB t=1 p=1",
        memory_kib: 46 * 1024,
        passes: 1,
        lanes: 1,
    },
];

fn argon2id(p: Profile, input: &[u8], salt: &[u8]) -> [u8; 32] {
    let params = Params::new(p.memory_kib, p.passes, p.lanes, Some(32)).expect("valid params");
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(input, salt, &mut out)
        .expect("argon2id");
    out
}

/// The last `$`-separated field of Cloaked's encoded hash string is the raw key.
fn key_field(encoded: &str) -> Result<Vec<u8>, String> {
    let field = encoded.rsplit('$').next().ok_or("empty hash string")?;
    B64.decode(field).map_err(|e| format!("bad base64 in hash string: {e}"))
}

fn verify(path: &str) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let field = |k: &str| {
        v[k].as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("missing \"{k}\""))
    };
    let password = field("password")?;
    let salt = B64
        .decode(field("account_salt")?)
        .map_err(|e| format!("bad account_salt: {e}"))?;
    let want_box = key_field(&field("secret_box_key")?)?;
    let want_auth = key_field(&field("auth_key")?)?;

    let got_box = argon2id(VAULT_KEY, password.as_bytes(), &salt);
    let got_auth = argon2id(LOGIN_KEY, &got_box, &salt);

    let report = |label: &str, got: &[u8], want: &[u8]| {
        let ok = got == want;
        println!("{} {label}: {}", if ok { "MATCH   " } else { "MISMATCH" }, B64.encode(got));
        ok
    };
    let box_ok = report("secret box key = Argon2id(password, salt; 64 MiB, t=1, p=4)", &got_box, &want_box);
    let auth_ok = report("auth key       = Argon2id(secret box key, salt; 64 MiB, t=5, p=4)", &got_auth, &want_auth);
    if box_ok && auth_ok { Ok(()) } else { Err("vectors did not reproduce".into()) }
}

fn time_ms(p: Profile, iterations: u32) -> f64 {
    let salt = [7u8; 32];
    let mut samples: Vec<f64> = (0..iterations)
        .map(|i| {
            let start = Instant::now();
            std::hint::black_box(argon2id(p, format!("guess-{i}").as_bytes(), &salt));
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn bench(iterations: u32) {
    println!("Single-threaded CPU time per guess (median of {iterations}).");
    println!("memory x passes is the hardware-independent cost an attacker pays.\n");
    println!("{:<22} {:>8} {:>6} {:>6} {:>10} {:>9}  note", "profile", "memory", "passes", "lanes", "median ms", "m x t");
    let base = VAULT_KEY.memory_kib as f64 * VAULT_KEY.passes as f64;
    for p in [VAULT_KEY, LOGIN_KEY].iter().chain(COMPARISONS.iter()) {
        let mt = p.memory_kib as f64 * p.passes as f64;
        println!(
            "{:<22} {:>5}MiB {:>6} {:>6} {:>10.1} {:>8.1}x  {}",
            p.name,
            p.memory_kib / 1024,
            p.passes,
            p.lanes,
            time_ms(*p, iterations),
            mt / base,
            p.note
        );
    }
    println!(
        "\nAn attacker with a database copy verifies guesses through the cheapest path.\n\
         The server stores the private key wrapped by the vault key, so the cheapest\n\
         path is one vault-key derivation (1.0x), not the login-key chain."
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify") => match args.get(1) {
            Some(path) => verify(path),
            None => Err("usage: kdf-cost verify <vectors.json>".into()),
        },
        Some("bench") => {
            let n = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(9);
            bench(n.max(1));
            Ok(())
        }
        _ => Err("usage: kdf-cost verify <vectors.json> | kdf-cost bench [iterations]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduces_published_vectors() {
        verify(concat!(env!("CARGO_MANIFEST_DIR"), "/../evidence/kdf-vectors.json")).unwrap();
    }
}
