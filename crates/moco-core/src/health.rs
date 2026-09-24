//! Vault health: practical, actionable findings. No scores to chase, no gamification.
//!
//! Following NIST SP 800-63B, age alone is informational — we never nag people to rotate
//! strong, unique passwords. Weak and reused passwords are what actually get accounts
//! taken over, so those lead.

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::collections::HashMap;
use uuid::Uuid;

use crate::crypto::random;
use crate::model::{now_ms, FieldType, Item, ItemKind, Timestamp};
use crate::strength;

const DAY: i64 = 24 * 3600 * 1000;
const OLD_DAYS: i64 = 3 * 365;
const EXPIRY_WINDOW_DAYS: i64 = 90;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    pub id: Uuid,
    pub title: String,
    pub kind: ItemKind,
    pub subtitle: String,
    pub urls: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub item: ItemRef,
    /// Human detail in PT-BR ("Muito fraca", "Vence em 12 dias"…).
    pub detail: String,
    /// Which field the finding is about.
    pub field: String,
    /// Days (age, or until expiry — negative when already expired).
    pub days: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReuseGroup {
    pub items: Vec<ItemRef>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub checked_at: Timestamp,
    pub total_items: usize,
    pub passwords: usize,
    pub weak: Vec<Finding>,
    pub reused: Vec<ReuseGroup>,
    pub insecure_sites: Vec<Finding>,
    pub expiring: Vec<Finding>,
    pub old: Vec<Finding>,
}

fn item_ref(i: &Item) -> ItemRef {
    ItemRef { id: i.id, title: i.overview.title.clone(), kind: i.kind, subtitle: i.overview.subtitle.clone(), urls: i.overview.urls.clone() }
}

/// Parses "MM/AA", "MM/AAAA" or ISO "AAAA-MM-DD" into a unix-ms end-of-validity.
fn expiry_ms(v: &str) -> Option<i64> {
    let v = v.trim();
    let days_from_civil = |y: i64, m: i64, d: i64| -> i64 {
        // Howard Hinnant's algorithm.
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    };
    if let Some((m, y)) = v.split_once('/') {
        let m: i64 = m.trim().parse().ok()?;
        let mut y: i64 = y.trim().parse().ok()?;
        if !(1..=12).contains(&m) {
            return None;
        }
        if y < 100 {
            y += 2000;
        }
        // Cards are valid through the last day of the month.
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        return Some(days_from_civil(ny, nm, 1) * DAY - 1);
    }
    let parts: Vec<&str> = v.split('-').collect();
    if parts.len() == 3 {
        let (y, m, d) = (parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?);
        return Some(days_from_civil(y, m, d) * DAY + DAY - 1);
    }
    None
}

fn expiry_field(kind: ItemKind) -> Option<&'static str> {
    match kind {
        ItemKind::Card => Some("expiry"),
        ItemKind::Document | ItemKind::License => Some("expiryDate"),
        ItemKind::HealthPlan => Some("validUntil"),
        ItemKind::ApiCredential => Some("expiresAt"),
        _ => None,
    }
}

pub fn analyze(items: &[Item]) -> HealthReport {
    let now = now_ms();
    let live: Vec<&Item> = items.iter().filter(|i| i.overview.trashed_at.is_none()).collect();
    let mut report = HealthReport { checked_at: now, total_items: live.len(), ..Default::default() };

    // Group identical passwords by a keyed hash so no plaintext set outlives this call.
    let key = random::bytes::<32>();
    let mut groups: HashMap<[u8; 32], Vec<&Item>> = HashMap::new();

    for item in &live {
        for fid in item.kind.password_fields() {
            let Some(f) = item.details.field(fid) else { continue };
            if f.kind != FieldType::Concealed || f.value.is_empty() {
                continue;
            }
            report.passwords += 1;
            let pw = f.value.expose();

            let mut inputs: Vec<&str> = vec![item.overview.title.as_str()];
            if let Some(u) = item.details.value("username") {
                inputs.push(u);
            }
            let s = strength::estimate(pw, &inputs);
            if s.score <= 1 {
                report.weak.push(Finding {
                    item: item_ref(item),
                    detail: format!("{} · quebraria em {}", s.label, s.crack_time),
                    field: (*fid).to_string(),
                    days: None,
                });
            }

            let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("any key size");
            mac.update(pw.as_bytes());
            let tag: [u8; 32] = mac.finalize().into_bytes().into();
            groups.entry(tag).or_default().push(item);

            // Age of the current password: when it last replaced another, else creation.
            let set_at = item.details.password_history.first().map(|h| h.replaced_at).unwrap_or(item.created_at);
            let age_days = (now - set_at) / DAY;
            if age_days >= OLD_DAYS {
                report.old.push(Finding {
                    item: item_ref(item),
                    detail: format!("Sem trocar há {} anos", age_days / 365),
                    field: (*fid).to_string(),
                    days: Some(age_days),
                });
            }
        }

        if item.kind == ItemKind::Login || item.kind == ItemKind::Server {
            if let Some(u) = item.overview.urls.iter().find(|u| u.trim().to_ascii_lowercase().starts_with("http://")) {
                report.insecure_sites.push(Finding {
                    item: item_ref(item),
                    detail: format!("{} não usa conexão segura (https)", crate::model::url_host(u)),
                    field: "url".into(),
                    days: None,
                });
            }
        }

        if let Some(fid) = expiry_field(item.kind) {
            if let Some(end) = item.details.value(fid).and_then(expiry_ms) {
                let days = (end - now).div_euclid(DAY);
                if days <= EXPIRY_WINDOW_DAYS {
                    let detail = if days < 0 {
                        format!("Venceu há {} {}", -days, if -days == 1 { "dia" } else { "dias" })
                    } else if days == 0 {
                        "Vence hoje".to_string()
                    } else {
                        format!("Vence em {} {}", days, if days == 1 { "dia" } else { "dias" })
                    };
                    report.expiring.push(Finding { item: item_ref(item), detail, field: fid.into(), days: Some(days) });
                }
            }
        }
    }

    report.reused = groups
        .into_values()
        .filter(|g| g.len() > 1)
        .map(|g| ReuseGroup { items: g.into_iter().map(item_ref).collect() })
        .collect();
    report.reused.sort_by(|a, b| b.items.len().cmp(&a.items.len()));
    report.weak.sort_by(|a, b| a.item.title.to_lowercase().cmp(&b.item.title.to_lowercase()));
    report.expiring.sort_by_key(|f| f.days);
    report.old.sort_by_key(|f| std::cmp::Reverse(f.days));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretString;
    use crate::model::{Details, Field, Overview};

    fn login(title: &str, pw: &str, url: &str) -> Item {
        Item {
            id: Uuid::new_v4(),
            vault_id: Uuid::nil(),
            kind: ItemKind::Login,
            overview: Overview { title: title.into(), urls: vec![url.into()], ..Default::default() },
            details: Details {
                fields: vec![Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new(pw), section: None }],
                ..Default::default()
            },
            created_at: now_ms(),
            updated_at: now_ms(),
            revision: 1,
        }
    }

    #[test]
    fn finds_weak_reused_and_insecure() {
        let items = vec![
            login("Netflix", "netflix123", "https://netflix.com"),
            login("Spotify", "netflix123", "https://spotify.com"),
            login("Banco", "q8#Lm2!vZr9@Tx4^", "http://banco.example"),
        ];
        let r = analyze(&items);
        assert_eq!(r.passwords, 3);
        assert_eq!(r.weak.len(), 2);
        assert_eq!(r.reused.len(), 1);
        assert_eq!(r.reused[0].items.len(), 2);
        assert_eq!(r.insecure_sites.len(), 1);
    }

    #[test]
    fn expiry_parsing() {
        let may_2030 = expiry_ms("05/30").unwrap();
        let june_first_2030 = expiry_ms("2030-06-01").unwrap();
        assert!(may_2030 < june_first_2030);
        assert!(expiry_ms("13/30").is_none());
        assert!(expiry_ms("xyz").is_none());
        let mut card = login("Cartão", "", "");
        card.kind = ItemKind::Card;
        card.details.fields = vec![Field { id: "expiry".into(), label: String::new(), kind: FieldType::MonthYear, value: SecretString::new("01/20"), section: None }];
        let r = analyze(&[card]);
        assert_eq!(r.expiring.len(), 1);
        assert!(r.expiring[0].detail.starts_with("Venceu há"));
    }
}
