//! Desktop OAuth primitives and current-user Windows DPAPI storage.
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub provider: usize,
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn nonce() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Windows random number generation failed."))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
pub fn callback(
    request: &str,
    expected_host: &str,
    expected_state: &str,
) -> Result<Option<String>> {
    let mut lines = request.split("\r\n");
    let Some(first) = lines.next() else {
        return Ok(None);
    };
    let mut parts = first.split_whitespace();
    if parts.next() != Some("GET") {
        return Ok(None);
    }
    let Some(target) = parts.next() else {
        return Ok(None);
    };
    if !target.starts_with("/oauth/callback?") {
        return Ok(None);
    }
    let host = lines
        .filter_map(|s| s.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim());
    if host != Some(expected_host) {
        return Ok(None);
    }
    let url = Url::parse(&format!("http://{expected_host}{target}"))?;
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || !pairs
            .iter()
            .any(|(k, v)| k == "state" && v == expected_state)
    {
        return Ok(None);
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        bail!("Google sign-in was declined. You can try again.");
    }
    let codes: Vec<_> = pairs.iter().filter(|(k, _)| k == "code").collect();
    if codes.len() != 1 || codes[0].1.is_empty() || codes[0].1.len() > 4096 {
        bail!("Google returned an invalid authorization response.");
    }
    Ok(Some(codes[0].1.to_string()))
}

pub fn seal(bytes: &[u8]) -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(protect(bytes, false)?))
}
pub fn unseal(value: &str) -> Result<Vec<u8>> {
    if value.len() > 65_536 {
        bail!("Saved login is invalid; sign in again.");
    }
    protect(
        &URL_SAFE_NO_PAD
            .decode(value)
            .context("Saved login could not be decoded; sign in again.")?,
        true,
    )
}
#[cfg(windows)]
fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::Cryptography::{
                CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
            },
        },
        core::w,
    };
    if bytes.len() > 49_152 {
        bail!("Login data is too large.");
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // DPAPI copies the input and returns a LocalAlloc buffer owned by us.
    unsafe {
        if decrypt {
            CryptUnprotectData(&input,None,None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output)
        } else {
            CryptProtectData(&input,w!("ARIA streaming chat login"),None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output)
        }.context("Windows could not protect or unlock this login. Sign in on this Windows account again.")?;
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        // Wipe the native plaintext buffer before freeing it after decryption.
        if decrypt {
            for i in 0..output.cbData as usize {
                std::ptr::write_volatile(output.pbData.add(i), 0);
            }
        }
        let _ = LocalFree(HLOCAL(output.pbData.cast()));
        Ok(result)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>> {
    bail!("Chat login storage currently requires Windows DPAPI.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(nonce().unwrap().len(), 43);
        assert_ne!(nonce().unwrap(), nonce().unwrap());
    }
    #[test]
    fn callback_checks_path_host_state_and_duplicates() {
        let req =
            |q: &str| format!("GET /oauth/callback?{q} HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n");
        assert_eq!(
            callback(&req("state=unique&code=a%2Fb"), "127.0.0.1:1234", "unique").unwrap(),
            Some("a/b".into())
        );
        for query in [
            "state=wrong&code=a",
            "code=a",
            "state=unique&state=unique&code=a",
        ] {
            assert!(
                callback(&req(query), "127.0.0.1:1234", "unique")
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            callback(&req("state=unique&code=a"), "evil.test", "unique")
                .unwrap()
                .is_none()
        );
        assert!(
            callback(
                &req("state=unique&error=access_denied"),
                "127.0.0.1:1234",
                "unique"
            )
            .is_err()
        );
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Requires a Windows user DPAPI profile"]
    fn windows_login_encryption_roundtrip() {
        let bytes = b"test-only-token-not-a-real-credential";
        let sealed = seal(bytes).unwrap();
        assert!(!sealed.contains("test-only"));
        assert_eq!(unseal(&sealed).unwrap(), bytes);
        assert!(unseal("not-a-valid-protected-token").is_err());
    }
}
