use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const SESSION_COOKIE_NAME: &str = "rustcnc_session";

const HASH_SCHEME_PREFIX: &str = "sha256-iter:v1";
const DEFAULT_ITERATIONS: u32 = 200_000;

#[derive(Debug, Clone)]
struct ParsedHash {
    iterations: u32,
    salt: Vec<u8>,
    hash: Vec<u8>,
}

fn parse_hash(s: &str) -> anyhow::Result<ParsedHash> {
    let mut parts = s.split(':');
    let scheme = parts.next().unwrap_or_default();
    let version = parts.next().unwrap_or_default();
    let iters = parts.next().unwrap_or_default();
    let salt_b64 = parts.next().unwrap_or_default();
    let hash_b64 = parts.next().unwrap_or_default();

    let scheme_version = format!("{}:{}", scheme, version);
    anyhow::ensure!(
        scheme_version == HASH_SCHEME_PREFIX,
        "Unsupported password hash scheme"
    );
    let iterations: u32 = iters.parse()?;
    anyhow::ensure!(iterations >= 10_000, "Iterations too low");

    let salt = URL_SAFE_NO_PAD.decode(salt_b64)?;
    let hash = URL_SAFE_NO_PAD.decode(hash_b64)?;
    anyhow::ensure!(!salt.is_empty(), "Salt missing");
    anyhow::ensure!(hash.len() == 32, "Unexpected hash length");

    Ok(ParsedHash {
        iterations,
        salt,
        hash,
    })
}

fn derive_sha256_iter(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut out: [u8; 32] = {
        let mut h = Sha256::new();
        h.update(salt);
        h.update(password.as_bytes());
        h.finalize().into()
    };

    // Simple iterative stretching. Not as strong as a memory-hard KDF,
    // but sufficient for local/LAN protection when argon2 isn't available.
    for _ in 1..iterations {
        let mut h = Sha256::new();
        h.update(out);
        h.update(salt);
        h.update(password.as_bytes());
        out = h.finalize().into();
    }

    out
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    anyhow::ensure!(!password.is_empty(), "Password must not be empty");

    let salt = Uuid::new_v4().as_bytes().to_vec();
    let iterations = DEFAULT_ITERATIONS;
    let derived = derive_sha256_iter(password, &salt, iterations);

    let salt_b64 = URL_SAFE_NO_PAD.encode(salt);
    let hash_b64 = URL_SAFE_NO_PAD.encode(derived);
    Ok(format!(
        "{}:{}:{}:{}",
        HASH_SCHEME_PREFIX, iterations, salt_b64, hash_b64
    ))
}

pub fn verify_password(password: &str, encoded: &str) -> anyhow::Result<bool> {
    let parsed = parse_hash(encoded)?;
    let derived = derive_sha256_iter(password, &parsed.salt, parsed.iterations);
    Ok(constant_time_eq(&derived, &parsed.hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_roundtrip() {
        let encoded = hash_password("secret").expect("hash");
        assert!(verify_password("secret", &encoded).expect("verify"));
        assert!(!verify_password("wrong", &encoded).expect("verify"));
    }
}
