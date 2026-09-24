//! Breached-password check via Have I Been Pwned's k-anonymity range API.
//!
//! Only the first 5 hex characters of each password's SHA-1 leave this computer; the
//! response lists every suffix in that range (with padding enabled, so response size
//! doesn't hint at the prefix) and the match happens locally. Opt-in, off by default.

use moco_core::model::{FieldType, Item};
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreachHit {
    pub item_id: Uuid,
    pub title: String,
    pub field: String,
    /// How many times this password appears in known breaches.
    pub count: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreachReport {
    pub checked: usize,
    pub hits: Vec<BreachHit>,
}

fn sha1_hex(s: &str) -> String {
    let d = Sha1::digest(s.as_bytes());
    d.iter().map(|b| format!("{b:02X}")).collect()
}

pub fn check(items: &[Item]) -> AppResult<BreachReport> {
    // prefix → [(suffix, item, field)]
    let mut by_prefix: HashMap<String, Vec<(String, &Item, String)>> = HashMap::new();
    let mut checked = 0;
    for item in items.iter().filter(|i| i.overview.trashed_at.is_none()) {
        for f in item.details.fields.iter().filter(|f| f.kind == FieldType::Concealed && !f.value.is_empty()) {
            let h = sha1_hex(f.value.expose());
            let (p, s) = h.split_at(5);
            by_prefix.entry(p.to_string()).or_default().push((s.to_string(), item, f.id.clone()));
            checked += 1;
        }
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .user_agent("Moco-Desktop (password health check)")
        .build()
        .into();

    let mut hits = Vec::new();
    for (prefix, entries) in by_prefix {
        let body = agent
            .get(&format!("https://api.pwnedpasswords.com/range/{prefix}"))
            .header("Add-Padding", "true")
            .call()
            .map_err(|_| AppError::new("offline", "Não deu para consultar a base de vazamentos agora. Verifique a internet e tente de novo."))?
            .body_mut()
            .read_to_string()
            .map_err(|e| AppError::new("offline", format!("Resposta inválida da base de vazamentos: {e}")))?;
        let counts: HashMap<&str, u64> = body
            .lines()
            .filter_map(|l| {
                let (suf, n) = l.trim().split_once(':')?;
                let n: u64 = n.trim().parse().ok()?;
                (n > 0).then_some((suf, n))
            })
            .collect();
        for (suffix, item, field) in entries {
            if let Some(&count) = counts.get(suffix.as_str()) {
                hits.push(BreachHit { item_id: item.id, title: item.overview.title.clone(), field, count });
            }
        }
    }
    hits.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(BreachReport { checked, hits })
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha1_matches_known_value() {
        assert_eq!(super::sha1_hex("password"), "5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8");
    }
}
