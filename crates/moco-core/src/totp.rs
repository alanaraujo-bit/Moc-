//! Time-based one-time passwords (RFC 6238 over RFC 4226 HOTP) and `otpauth://` parsing.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Algorithm {
    #[default]
    #[serde(rename = "SHA1")]
    Sha1,
    #[serde(rename = "SHA256")]
    Sha256,
    #[serde(rename = "SHA512")]
    Sha512,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotpConfig {
    pub secret: Vec<u8>,
    pub algorithm: Algorithm,
    pub digits: u32,
    pub period: u64,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub period: u64,
    /// Seconds until the code rotates.
    pub remaining: u64,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

fn decode_base32(s: &str) -> Option<Vec<u8>> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace() && *c != '-').map(|c| c.to_ascii_uppercase()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    let mut padded = trimmed.to_string();
    while padded.len() % 8 != 0 {
        padded.push('=');
    }
    data_encoding::BASE32.decode(padded.as_bytes()).ok().filter(|v| !v.is_empty())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl TotpConfig {
    /// Accepts an `otpauth://totp/...` URI or a bare Base32 secret.
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        if let Some(rest) = input.strip_prefix("otpauth://") {
            let (kind, rest) = rest.split_once('/').ok_or_else(|| CoreError::Invalid("URI otpauth inválida".into()))?;
            if !kind.eq_ignore_ascii_case("totp") {
                return Err(CoreError::Invalid("só códigos baseados em tempo (TOTP) são suportados".into()));
            }
            let (label, query) = rest.split_once('?').unwrap_or((rest, ""));
            let label = percent_decode(label);
            let (label_issuer, account) = match label.split_once(':') {
                Some((i, a)) => (Some(i.trim().to_string()), Some(a.trim().to_string())),
                None => (None, Some(label.trim().to_string()).filter(|s| !s.is_empty())),
            };
            let mut cfg = TotpConfig { secret: vec![], algorithm: Algorithm::Sha1, digits: 6, period: 30, issuer: label_issuer, account };
            for pair in query.split('&').filter(|p| !p.is_empty()) {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                let v = percent_decode(v);
                match k.to_ascii_lowercase().as_str() {
                    "secret" => cfg.secret = decode_base32(&v).ok_or_else(|| CoreError::Invalid("chave secreta do 2FA inválida".into()))?,
                    "algorithm" => {
                        cfg.algorithm = match v.to_ascii_uppercase().as_str() {
                            "SHA1" => Algorithm::Sha1,
                            "SHA256" => Algorithm::Sha256,
                            "SHA512" => Algorithm::Sha512,
                            _ => return Err(CoreError::Invalid("algoritmo de 2FA não suportado".into())),
                        }
                    }
                    "digits" => cfg.digits = v.parse().map_err(|_| CoreError::Invalid("dígitos inválidos".into()))?,
                    "period" => cfg.period = v.parse().map_err(|_| CoreError::Invalid("período inválido".into()))?,
                    "issuer" if !v.is_empty() => cfg.issuer = Some(v),
                    _ => {}
                }
            }
            if cfg.secret.is_empty() {
                return Err(CoreError::Invalid("a URI não tem a chave secreta".into()));
            }
            cfg.validate()?;
            Ok(cfg)
        } else {
            let secret = decode_base32(input).ok_or_else(|| CoreError::Invalid("isso não parece uma chave de 2FA".into()))?;
            let cfg = TotpConfig { secret, algorithm: Algorithm::Sha1, digits: 6, period: 30, issuer: None, account: None };
            cfg.validate()?;
            Ok(cfg)
        }
    }

    fn validate(&self) -> Result<()> {
        if !(6..=10).contains(&self.digits) || !(10..=300).contains(&self.period) || self.secret.len() < 10 {
            return Err(CoreError::Invalid("configuração de 2FA fora do padrão".into()));
        }
        Ok(())
    }

    pub fn hotp(&self, counter: u64) -> String {
        let msg = counter.to_be_bytes();
        let digest: Vec<u8> = match self.algorithm {
            Algorithm::Sha1 => {
                let mut m = Hmac::<sha1::Sha1>::new_from_slice(&self.secret).expect("any key length");
                m.update(&msg);
                m.finalize().into_bytes().to_vec()
            }
            Algorithm::Sha256 => {
                let mut m = Hmac::<sha2::Sha256>::new_from_slice(&self.secret).expect("any key length");
                m.update(&msg);
                m.finalize().into_bytes().to_vec()
            }
            Algorithm::Sha512 => {
                let mut m = Hmac::<sha2::Sha512>::new_from_slice(&self.secret).expect("any key length");
                m.update(&msg);
                m.finalize().into_bytes().to_vec()
            }
        };
        let offset = (digest[digest.len() - 1] & 0x0f) as usize;
        let bin = ((digest[offset] as u32 & 0x7f) << 24)
            | ((digest[offset + 1] as u32) << 16)
            | ((digest[offset + 2] as u32) << 8)
            | digest[offset + 3] as u32;
        let modulo = 10u64.pow(self.digits);
        format!("{:0width$}", bin as u64 % modulo, width = self.digits as usize)
    }

    pub fn code_at(&self, unix_seconds: u64) -> TotpCode {
        TotpCode {
            code: self.hotp(unix_seconds / self.period),
            period: self.period,
            remaining: self.period - unix_seconds % self.period,
            issuer: self.issuer.clone(),
            account: self.account.clone(),
        }
    }

    pub fn now(&self) -> TotpCode {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        self.code_at(secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc(alg: Algorithm, secret: &[u8]) -> TotpConfig {
        TotpConfig { secret: secret.to_vec(), algorithm: alg, digits: 8, period: 30, issuer: None, account: None }
    }

    #[test]
    fn rfc6238_vectors() {
        let s1 = b"12345678901234567890";
        let s256 = b"12345678901234567890123456789012";
        let s512 = b"1234567890123456789012345678901234567890123456789012345678901234";
        let cases = [
            (59u64, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (1111111111, "14050471", "67062674", "99943326"),
            (1234567890, "89005924", "91819424", "93441116"),
            (2000000000, "69279037", "90698825", "38618901"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        for (t, c1, c256, c512) in cases {
            assert_eq!(rfc(Algorithm::Sha1, s1).code_at(t).code, c1);
            assert_eq!(rfc(Algorithm::Sha256, s256).code_at(t).code, c256);
            assert_eq!(rfc(Algorithm::Sha512, s512).code_at(t).code, c512);
        }
    }

    #[test]
    fn parses_uris_and_bare_secrets() {
        let c = TotpConfig::parse("otpauth://totp/Nubank:ana%40email.com?secret=JBSWY3DPEHPK3PXP&issuer=Nubank&digits=6&period=30").unwrap();
        assert_eq!(c.issuer.as_deref(), Some("Nubank"));
        assert_eq!(c.account.as_deref(), Some("ana@email.com"));
        assert_eq!(c.digits, 6);
        let bare = TotpConfig::parse("jbsw y3dp ehpk 3pxp").unwrap();
        assert_eq!(bare.secret, c.secret);
        assert!(TotpConfig::parse("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
        assert!(TotpConfig::parse("não é base32!").is_err());
        let code = bare.code_at(59);
        assert_eq!(code.code.len(), 6);
        assert_eq!(code.remaining, 1);
    }
}
