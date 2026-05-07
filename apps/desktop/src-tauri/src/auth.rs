//! Holster Auth — local-first TOTP helpers.
//!
//! V0 deliberately stores authenticator entries as ordinary encrypted vault
//! records with a reserved project tag. That keeps the first version inside
//! the existing vault threat model and avoids a schema migration.
//!
//! Plain TOTP secrets never cross IPC. The UI can submit a secret once when
//! adding an account, then later request only the current 6-digit code.

use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use url::Url;

pub const AUTH_PROJECT_TAG: &str = "__holster_auth_totp";
pub const AUTH_VALUE_VERSION: u8 = 1;

type HmacSha1 = Hmac<Sha1>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthSecretRecord {
    pub version: u8,
    pub secret_base32: String,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub backup_codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TotpAccountDto {
    pub id: String,
    pub label: String,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub backup_code_count: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AddTotpAccountArgs {
    pub label: String,
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub secret_or_uri: String,
    pub backup_codes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TotpCodeReport {
    pub code: String,
    pub seconds_remaining: u64,
    pub period: u64,
}

pub fn parse_backup_codes(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn normalize_auth_input(
    secret_or_uri: &str,
    issuer: Option<&str>,
    account_name: Option<&str>,
) -> Result<AuthSecretRecord, String> {
    let trimmed = secret_or_uri.trim();
    if trimmed.is_empty() {
        return Err("enter a TOTP secret or otpauth:// URI".into());
    }

    let mut parsed_issuer = issuer
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let mut parsed_account = account_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let secret = if trimmed.starts_with("otpauth://") {
        let url = Url::parse(trimmed).map_err(|_| "invalid otpauth URI".to_string())?;
        if url.host_str() != Some("totp") {
            return Err("only otpauth://totp URIs are supported".into());
        }
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "secret" => {}
                "issuer" if parsed_issuer.is_none() => parsed_issuer = Some(value.to_string()),
                _ => {}
            }
        }
        if parsed_account.is_none() {
            let label = url.path().trim_start_matches('/');
            if !label.is_empty() {
                let decoded = percent_decode(label);
                if let Some((uri_issuer, acct)) = decoded.split_once(':') {
                    if parsed_issuer.is_none() && !uri_issuer.trim().is_empty() {
                        parsed_issuer = Some(uri_issuer.trim().to_string());
                    }
                    if !acct.trim().is_empty() {
                        parsed_account = Some(acct.trim().to_string());
                    }
                } else {
                    parsed_account = Some(decoded);
                }
            }
        }
        url.query_pairs()
            .find(|(key, _)| key == "secret")
            .map(|(_, value)| value.to_string())
            .ok_or_else(|| "otpauth URI is missing a secret".to_string())?
    } else {
        trimmed.to_string()
    };

    let canonical_secret = canonical_base32_secret(&secret)?;
    Ok(AuthSecretRecord {
        version: AUTH_VALUE_VERSION,
        secret_base32: canonical_secret,
        issuer: parsed_issuer,
        account_name: parsed_account,
        backup_codes: Vec::new(),
    })
}

pub fn record_to_secret_string(record: &AuthSecretRecord) -> Result<String, String> {
    serde_json::to_string(record).map_err(|e| format!("could not serialize auth record: {e}"))
}

pub fn secret_string_to_record(secret: &Secret<String>) -> Result<AuthSecretRecord, String> {
    serde_json::from_str(secret.expose_secret()).map_err(|_| "auth record is corrupted".to_string())
}

pub fn current_totp(secret_base32: &str, now_unix: u64) -> Result<TotpCodeReport, String> {
    let secret = decode_base32(secret_base32)?;
    let period = 30_u64;
    let counter = now_unix / period;
    let seconds_remaining = period - (now_unix % period);
    let code = hotp_sha1(&secret, counter, 6)?;
    Ok(TotpCodeReport {
        code,
        seconds_remaining,
        period,
    })
}

fn hotp_sha1(secret: &[u8], counter: u64, digits: u32) -> Result<String, String> {
    let mut mac = HmacSha1::new_from_slice(secret).map_err(|_| "invalid HMAC key".to_string())?;
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();
    let offset = (result[19] & 0x0f) as usize;
    let binary = (((result[offset] & 0x7f) as u32) << 24)
        | ((result[offset + 1] as u32) << 16)
        | ((result[offset + 2] as u32) << 8)
        | (result[offset + 3] as u32);
    let modulo = 10_u32.pow(digits);
    Ok(format!(
        "{:0width$}",
        binary % modulo,
        width = digits as usize
    ))
}

fn canonical_base32_secret(raw: &str) -> Result<String, String> {
    let canonical: String = raw
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    let _ = decode_base32(&canonical)?;
    Ok(canonical)
}

fn decode_base32(raw: &str) -> Result<Vec<u8>, String> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();

    for ch in raw.chars().filter(|ch| *ch != '=') {
        let value = match ch {
            'A'..='Z' => (ch as u8 - b'A') as u32,
            '2'..='7' => (ch as u8 - b'2' + 26) as u32,
            _ => return Err("TOTP secret must be base32 characters A-Z and 2-7".into()),
        };
        bits = (bits << 5) | value;
        bit_count += 5;
        while bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }

    if out.is_empty() {
        return Err("TOTP secret decoded to empty bytes".into());
    }
    Ok(out)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_6238_sha1_vector_at_59_seconds() {
        // RFC 6238 test secret "12345678901234567890" in base32.
        let report = current_totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
        assert_eq!(report.code, "287082");
        assert_eq!(report.seconds_remaining, 1);
    }

    #[test]
    fn parses_otpauth_uri_without_leaking_secret_in_metadata() {
        let record = normalize_auth_input(
            "otpauth://totp/Cloudflare:dave@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Cloudflare",
            None,
            None,
        )
        .unwrap();
        assert_eq!(record.issuer.as_deref(), Some("Cloudflare"));
        assert_eq!(record.account_name.as_deref(), Some("dave@example.com"));
        assert_eq!(record.secret_base32, "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn backup_codes_parse_linewise() {
        let codes = parse_backup_codes(Some(" one \n\n two\nthree "));
        assert_eq!(codes, vec!["one", "two", "three"]);
    }
}
