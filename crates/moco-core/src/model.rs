//! Vault item model. Every client (desktop, extension, mobile) shares these shapes.
//!
//! An item is stored as two encrypted blobs:
//! - [`Overview`] — what lists and search need (title, subtitle, URLs, tags, flags);
//! - [`Details`] — fields, notes, history, attachments metadata.
//!
//! Field *layouts* (labels, masks, order) live in the UI templates; the core only knows the
//! semantic field ids it needs (e.g. which field is "the password") so every client
//! computes subtitles and health checks identically.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::SecretString;

pub type Timestamp = i64; // Unix milliseconds, UTC.

pub fn now_ms() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Login,
    Password,
    Card,
    Identity,
    Document,
    Note,
    Wifi,
    License,
    BankAccount,
    Server,
    Database,
    ApiCredential,
    SshKey,
    CryptoWallet,
    HealthPlan,
    Vehicle,
    Custom,
}

impl ItemKind {
    pub const ALL: [ItemKind; 17] = [
        ItemKind::Login,
        ItemKind::Password,
        ItemKind::Card,
        ItemKind::Identity,
        ItemKind::Document,
        ItemKind::Note,
        ItemKind::Wifi,
        ItemKind::License,
        ItemKind::BankAccount,
        ItemKind::Server,
        ItemKind::Database,
        ItemKind::ApiCredential,
        ItemKind::SshKey,
        ItemKind::CryptoWallet,
        ItemKind::HealthPlan,
        ItemKind::Vehicle,
        ItemKind::Custom,
    ];

    /// Field ids that hold this kind's main secret — used by health checks.
    pub fn password_fields(self) -> &'static [&'static str] {
        match self {
            ItemKind::Login | ItemKind::Password | ItemKind::Wifi | ItemKind::Server | ItemKind::Database => {
                &["password"]
            }
            _ => &[],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FieldType {
    Text,
    Concealed,
    Email,
    Url,
    Phone,
    Date,
    MonthYear,
    Number,
    Pin,
    Totp,
    Multiline,
    Select,
    CardNumber,
    Cpf,
    Cnpj,
    Cep,
    SshPrivateKey,
    SeedPhrase,
}

impl FieldType {
    /// Values of these types are never sent to the UI unless explicitly revealed/edited.
    pub fn is_concealed(self) -> bool {
        matches!(
            self,
            FieldType::Concealed | FieldType::Pin | FieldType::Totp | FieldType::SshPrivateKey | FieldType::SeedPhrase
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// Stable id. Template fields use canonical ids ("username", "password", "number"…);
    /// custom fields use a random id.
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    pub value: SecretString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordHistoryEntry {
    pub value: SecretString,
    pub replaced_at: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub mime: String,
    pub added_at: Timestamp,
}

/// Imported passkey metadata (the private key stays in `Details`; usable once the browser
/// extension exists).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Passkey {
    pub credential_id: String,
    pub rp_id: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub user_handle: String,
    pub private_key: SecretString,
    #[serde(default)]
    pub created_at: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<Timestamp>,
    /// Optional icon override: an emoji or a named glyph chosen by the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Searchable, non-secret hints extracted from fields (e.g. card brand, bank name).
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub has_totp: bool,
    #[serde(default)]
    pub attachment_count: u32,
    #[serde(default)]
    pub has_passkey: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub notes: SecretString,
    #[serde(default)]
    pub password_history: Vec<PasswordHistoryEntry>,
    #[serde(default)]
    pub attachments: Vec<AttachmentMeta>,
    #[serde(default)]
    pub passkeys: Vec<Passkey>,
}

impl Details {
    pub fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == id)
    }

    pub fn value(&self, id: &str) -> Option<&str> {
        self.field(id).map(|f| f.value.expose()).filter(|v| !v.is_empty())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub kind: ItemKind,
    pub overview: Overview,
    pub details: Details,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub revision: u64,
}

/// What callers provide to create or update an item.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemInput {
    pub kind: ItemKind,
    pub title: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub notes: SecretString,
}

/// Non-secret projection used by lists and search.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummary {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub kind: ItemKind,
    pub title: String,
    pub subtitle: String,
    pub urls: Vec<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub archived: bool,
    pub trashed_at: Option<Timestamp>,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub has_totp: bool,
    pub has_passkey: bool,
    pub attachment_count: u32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Item {
    pub fn summary(&self) -> ItemSummary {
        let o = &self.overview;
        ItemSummary {
            id: self.id,
            vault_id: self.vault_id,
            kind: self.kind,
            title: o.title.clone(),
            subtitle: o.subtitle.clone(),
            urls: o.urls.clone(),
            tags: o.tags.clone(),
            favorite: o.favorite,
            archived: o.archived,
            trashed_at: o.trashed_at,
            icon: o.icon.clone(),
            keywords: o.keywords.clone(),
            has_totp: o.has_totp,
            has_passkey: o.has_passkey,
            attachment_count: o.attachment_count,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Recomputes derived overview fields from details. Call after every edit.
    pub fn refresh_derived(&mut self) {
        let d = &self.details;
        self.overview.subtitle = subtitle_for(self.kind, d);
        self.overview.keywords = keywords_for(self.kind, d);
        self.overview.has_totp = d.fields.iter().any(|f| f.kind == FieldType::Totp && !f.value.is_empty());
        self.overview.attachment_count = d.attachments.len() as u32;
        self.overview.has_passkey = !d.passkeys.is_empty();
        self.overview.tags = normalize_tags(&self.overview.tags);
        self.overview.urls = self
            .overview
            .urls
            .iter()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect();
    }
}

/// Tags are trimmed, de-duplicated case-insensitively and may nest with "/".
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let clean = t
            .split('/')
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if !clean.is_empty() && !out.iter().any(|o| o.to_lowercase() == clean.to_lowercase()) {
            out.push(clean);
        }
    }
    out
}

fn last_digits(s: &str, n: usize) -> String {
    let digits: Vec<char> = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let start = digits.len().saturating_sub(n);
    digits[start..].iter().collect()
}

pub fn card_brand(number: &str) -> Option<&'static str> {
    let d: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    let p = |n: usize| d.get(..n).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    // Elo and Hipercard first: their ranges overlap others and they're common in Brazil.
    const ELO: [u32; 18] = [
        401178, 401179, 431274, 438935, 451416, 457393, 457631, 457632, 504175, 506699, 506770, 509000, 627780, 636297,
        636368, 650031, 650901, 651652,
    ];
    if d.len() >= 6 && (ELO.contains(&p(6)) || (650000..=650999).contains(&p(6)) || (506700..=506799).contains(&p(6)) || (509000..=509999).contains(&p(6))) {
        return Some("Elo");
    }
    if d.starts_with("606282") || d.starts_with("3841") {
        return Some("Hipercard");
    }
    if d.starts_with('4') {
        return Some("Visa");
    }
    if (51..=55).contains(&p(2)) || (2221..=2720).contains(&p(4)) {
        return Some("Mastercard");
    }
    if p(2) == 34 || p(2) == 37 {
        return Some("American Express");
    }
    if p(2) == 36 || p(2) == 38 || (300..=305).contains(&p(3)) {
        return Some("Diners Club");
    }
    if d.starts_with("6011") || d.starts_with("65") || (644..=649).contains(&p(3)) {
        return Some("Discover");
    }
    if (3528..=3589).contains(&p(4)) {
        return Some("JCB");
    }
    None
}

fn first_line(s: &str, max: usize) -> String {
    let line = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out: String = line.chars().take(max).collect();
    if line.chars().count() > max {
        out.push('…');
    }
    out
}

fn host_of(url: &str) -> String {
    let s = url.trim();
    let s = s.split("://").nth(1).unwrap_or(s);
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    host.trim_start_matches("www.").to_string()
}

pub fn subtitle_for(kind: ItemKind, d: &Details) -> String {
    let v = |id: &str| d.value(id).unwrap_or("").trim().to_string();
    let join = |parts: Vec<String>| parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ");
    match kind {
        ItemKind::Login => v("username"),
        ItemKind::Password => String::new(),
        ItemKind::Card => {
            let num = v("number");
            let last = last_digits(&num, 4);
            let brand = card_brand(&num).unwrap_or("").to_string();
            join(vec![brand, if last.is_empty() { String::new() } else { format!("•••• {last}") }])
        }
        ItemKind::Identity => {
            let name = v("fullName");
            if name.is_empty() { v("email") } else { name }
        }
        ItemKind::Document => join(vec![v("docType"), v("fullName")]),
        ItemKind::Note => first_line(d.notes.expose(), 64),
        ItemKind::Wifi => v("ssid"),
        ItemKind::License => join(vec![v("product"), v("version")]),
        ItemKind::BankAccount => {
            let branch = v("branch");
            let acct = v("accountNumber");
            join(vec![
                v("bank"),
                if branch.is_empty() { String::new() } else { format!("Ag. {branch}") },
                if acct.is_empty() { String::new() } else { format!("Conta {acct}") },
            ])
        }
        ItemKind::Server => {
            let (u, h) = (v("username"), v("host"));
            match (u.is_empty(), h.is_empty()) {
                (false, false) => format!("{u}@{h}"),
                (true, false) => h,
                _ => u,
            }
        }
        ItemKind::Database => join(vec![v("dbType"), v("host")]),
        ItemKind::ApiCredential => join(vec![v("service"), v("environment")]),
        ItemKind::SshKey => join(vec![v("keyType"), v("fingerprint")]),
        ItemKind::CryptoWallet => join(vec![v("network"), v("walletName")]),
        ItemKind::HealthPlan => join(vec![v("provider"), v("plan")]),
        ItemKind::Vehicle => join(vec![v("model"), v("plate")]),
        ItemKind::Custom => d
            .fields
            .iter()
            .find(|f| !f.kind.is_concealed() && !f.value.is_empty())
            .map(|f| first_line(f.value.expose(), 64))
            .unwrap_or_default(),
    }
}

fn keywords_for(kind: ItemKind, d: &Details) -> Vec<String> {
    let mut k = Vec::new();
    let mut push = |s: &str| {
        let s = s.trim();
        if !s.is_empty() && !k.iter().any(|x: &String| x == s) {
            k.push(s.to_string());
        }
    };
    match kind {
        ItemKind::Card => {
            if let Some(b) = card_brand(d.value("number").unwrap_or("")) {
                push(b);
            }
            for id in ["issuer", "cardholder", "cardType"] {
                push(d.value(id).unwrap_or(""));
            }
        }
        ItemKind::Login => {
            push(d.value("username").unwrap_or(""));
        }
        _ => {}
    }
    // Plain, non-secret text fields are searchable too (bank name, host, plate…).
    for f in &d.fields {
        if matches!(f.kind, FieldType::Text | FieldType::Email | FieldType::Select | FieldType::Url) {
            let v = f.value.expose();
            if v.chars().count() <= 80 {
                push(v);
            }
        }
    }
    k
}

pub fn url_host(url: &str) -> String {
    host_of(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(id: &str, kind: FieldType, v: &str) -> Field {
        Field { id: id.into(), label: String::new(), kind, value: SecretString::new(v), section: None }
    }

    #[test]
    fn brands() {
        assert_eq!(card_brand("4111 1111 1111 1111"), Some("Visa"));
        assert_eq!(card_brand("5555555555554444"), Some("Mastercard"));
        assert_eq!(card_brand("2223003122003222"), Some("Mastercard"));
        assert_eq!(card_brand("378282246310005"), Some("American Express"));
        assert_eq!(card_brand("6362970000457013"), Some("Elo"));
        assert_eq!(card_brand("6062825624254001"), Some("Hipercard"));
        assert_eq!(card_brand("1234"), None);
    }

    #[test]
    fn subtitles() {
        let d = Details { fields: vec![field("number", FieldType::CardNumber, "5555 5555 5555 4444")], ..Default::default() };
        assert_eq!(subtitle_for(ItemKind::Card, &d), "Mastercard · •••• 4444");
        let d = Details {
            fields: vec![field("username", FieldType::Text, "deploy"), field("host", FieldType::Text, "10.0.0.2")],
            ..Default::default()
        };
        assert_eq!(subtitle_for(ItemKind::Server, &d), "deploy@10.0.0.2");
        let d = Details { notes: SecretString::new("\n  Lista de presentes do Natal\nlinha 2"), ..Default::default() };
        assert_eq!(subtitle_for(ItemKind::Note, &d), "Lista de presentes do Natal");
    }

    #[test]
    fn tags_normalized() {
        let t = normalize_tags(&[" Trabalho / Clientes ".into(), "trabalho/clientes".into(), "".into(), "Casa".into()]);
        assert_eq!(t, vec!["Trabalho/Clientes".to_string(), "Casa".to_string()]);
    }

    #[test]
    fn hosts() {
        assert_eq!(url_host("https://www.nubank.com.br/login?x=1"), "nubank.com.br");
        assert_eq!(url_host("github.com"), "github.com");
    }

    #[test]
    fn serde_shape_is_camel_case() {
        let f = field("username", FieldType::Text, "ana");
        let j = serde_json::to_value(&f).unwrap();
        assert_eq!(j["type"], "text");
        assert_eq!(j["value"], "ana");
    }
}
