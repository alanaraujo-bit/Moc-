//! Importers for other password managers and browsers.
//!
//! Every importer produces [`Candidate`]s: fully-formed items plus where they came from
//! (folder/vault in the source) and any issue worth showing. Nothing is written here —
//! the caller previews, then commits. Parsing happens entirely in memory; plaintext
//! buffers are zeroized when dropped where practical.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use zeroize::Zeroizing;

use crate::crypto::SecretString;
use crate::error::{CoreError, Result};
use crate::model::{now_ms, url_host, Details, Field, FieldType, Item, ItemKind, Overview, Passkey, PasswordHistoryEntry};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Auto,
    Chrome,
    Firefox,
    Bitwarden,
    OnePassword,
    LastPass,
    ProtonPass,
    KeePass,
    Moco,
    GenericCsv,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Auto => "Detectar automaticamente",
            Source::Chrome => "Chrome, Edge ou Brave",
            Source::Firefox => "Firefox",
            Source::Bitwarden => "Bitwarden",
            Source::OnePassword => "1Password",
            Source::LastPass => "LastPass",
            Source::ProtonPass => "Proton Pass",
            Source::KeePass => "KeePass / KeePassXC",
            Source::Moco => "Mocó",
            Source::GenericCsv => "Planilha CSV",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub item: Item,
    /// Folder / vault / collection in the source, if any.
    pub folder: Option<String>,
    pub issue: Option<String>,
}

#[derive(Debug, Default)]
pub struct Parsed {
    pub source: Option<Source>,
    pub candidates: Vec<Candidate>,
    /// Entries we could not turn into items (with a reason).
    pub skipped: Vec<(String, String)>,
}

fn s(v: &str) -> SecretString {
    SecretString::new(v.trim_end_matches(['\r', '\n']).to_string())
}

fn field(id: &str, kind: FieldType, v: &str) -> Option<Field> {
    let v = v.trim();
    (!v.is_empty()).then(|| Field { id: id.into(), label: String::new(), kind, value: s(v), section: None })
}

fn custom(label: &str, kind: FieldType, v: &str) -> Option<Field> {
    let v = v.trim();
    (!v.is_empty()).then(|| Field { id: String::new(), label: label.trim().to_string(), kind, value: s(v), section: None })
}

fn new_item(kind: ItemKind, title: &str) -> Item {
    Item {
        id: uuid::Uuid::nil(),
        vault_id: uuid::Uuid::nil(),
        kind,
        overview: Overview { title: title.trim().to_string(), ..Default::default() },
        details: Details::default(),
        created_at: 0,
        updated_at: 0,
        revision: 1,
    }
}

fn title_from_url(url: &str) -> String {
    let host = url_host(url);
    let parts: Vec<&str> = host.split('.').collect();
    let brand = if parts.len() >= 3 && ["com", "net", "org", "gov", "edu", "co"].contains(&parts[parts.len() - 2]) {
        parts[parts.len() - 3]
    } else if parts.len() >= 2 {
        parts[parts.len() - 2]
    } else {
        host.as_str()
    };
    let mut c = brand.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Sem nome".into(),
    }
}

fn login(title: &str, url: &str, user: &str, pass: &str, notes: &str, totp: &str) -> Item {
    let title = if title.trim().is_empty() { title_from_url(url) } else { title.trim().to_string() };
    let mut it = new_item(ItemKind::Login, &title);
    if !url.trim().is_empty() {
        it.overview.urls = url.split(['\n', ',']).map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).collect();
    }
    it.details.fields = [field("username", FieldType::Text, user), field("password", FieldType::Concealed, pass), field("totp", FieldType::Totp, totp)]
        .into_iter()
        .flatten()
        .collect();
    it.details.notes = s(notes);
    it
}

fn note(title: &str, text: &str) -> Item {
    let mut it = new_item(ItemKind::Note, if title.trim().is_empty() { "Nota importada" } else { title });
    it.details.notes = s(text);
    it
}

// ---- CSV ------------------------------------------------------------------------------

struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn col(&self, names: &[&str]) -> Option<usize> {
        names.iter().find_map(|n| self.headers.iter().position(|h| h == n))
    }
}

fn read_csv(data: &[u8]) -> Result<Table> {
    let text = String::from_utf8_lossy(data);
    let text = text.trim_start_matches('\u{feff}');
    // Excel in pt-BR saves with ';'. Pick the delimiter that splits the header most.
    let first = text.lines().next().unwrap_or("");
    let delim = if first.matches(';').count() > first.matches(',').count() { b';' } else { b',' };
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).flexible(true).has_headers(true).from_reader(text.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| CoreError::Invalid(format!("CSV ilegível: {e}")))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    let mut rows = Vec::new();
    for r in rdr.records() {
        let r = r.map_err(|e| CoreError::Invalid(format!("CSV ilegível: {e}")))?;
        rows.push(r.iter().map(|c| c.to_string()).collect());
    }
    Ok(Table { headers, rows })
}

fn get<'a>(row: &'a [String], i: Option<usize>) -> &'a str {
    i.and_then(|i| row.get(i)).map(String::as_str).unwrap_or("")
}

fn parse_ms(v: &str) -> i64 {
    let v = v.trim();
    if let Ok(n) = v.parse::<i64>() {
        // seconds vs milliseconds vs microseconds (Chrome) heuristics
        return if n > 10_000_000_000_000 { n / 1000 } else if n > 10_000_000_000 { n } else { n * 1000 };
    }
    0
}

fn csv_chrome(t: &Table, out: &mut Parsed) {
    let (name, url, user, pass, notes) =
        (t.col(&["name"]), t.col(&["url"]), t.col(&["username"]), t.col(&["password"]), t.col(&["note", "notes"]));
    for row in &t.rows {
        let it = login(get(row, name), get(row, url), get(row, user), get(row, pass), get(row, notes), "");
        push_login(out, it, None, get(row, name));
    }
}

fn csv_firefox(t: &Table, out: &mut Parsed) {
    let (url, user, pass, created, changed) =
        (t.col(&["url"]), t.col(&["username"]), t.col(&["password"]), t.col(&["timecreated"]), t.col(&["timepasswordchanged"]));
    for row in &t.rows {
        let mut it = login("", get(row, url), get(row, user), get(row, pass), "", "");
        it.created_at = parse_ms(get(row, created));
        it.updated_at = parse_ms(get(row, changed)).max(it.created_at);
        push_login(out, it, None, get(row, url));
    }
}

fn csv_lastpass(t: &Table, out: &mut Parsed) {
    let (url, user, pass, totp, extra, name, group, fav) = (
        t.col(&["url"]),
        t.col(&["username"]),
        t.col(&["password"]),
        t.col(&["totp"]),
        t.col(&["extra"]),
        t.col(&["name"]),
        t.col(&["grouping"]),
        t.col(&["fav"]),
    );
    for row in &t.rows {
        let u = get(row, url);
        let folder = Some(get(row, group).trim().to_string()).filter(|g| !g.is_empty());
        // LastPass marks secure notes with url "http://sn".
        if u.trim() == "http://sn" {
            let mut it = note(get(row, name), get(row, extra));
            it.overview.favorite = get(row, fav) == "1";
            out.candidates.push(Candidate { item: it, folder, issue: None });
            continue;
        }
        let mut it = login(get(row, name), u, get(row, user), get(row, pass), get(row, extra), get(row, totp));
        it.overview.favorite = get(row, fav) == "1";
        push_login(out, it, folder, get(row, name));
    }
}

fn csv_bitwarden(t: &Table, out: &mut Parsed) {
    let c = |n: &str| t.col(&[n]);
    let (folder, fav, typ, name, notes, fields, uri, user, pass, totp) =
        (c("folder"), c("favorite"), c("type"), c("name"), c("notes"), c("fields"), c("login_uri"), c("login_username"), c("login_password"), c("login_totp"));
    for row in &t.rows {
        let f = Some(get(row, folder).trim().to_string()).filter(|g| !g.is_empty());
        let mut it = if get(row, typ) == "note" {
            note(get(row, name), get(row, notes))
        } else {
            login(get(row, name), get(row, uri), get(row, user), get(row, pass), get(row, notes), get(row, totp))
        };
        for line in get(row, fields).lines() {
            if let Some((k, v)) = line.split_once(": ") {
                it.details.fields.extend(custom(k, FieldType::Text, v));
            }
        }
        it.overview.favorite = get(row, fav) == "1";
        if it.kind == ItemKind::Login {
            push_login(out, it, f, get(row, name));
        } else {
            out.candidates.push(Candidate { item: it, folder: f, issue: None });
        }
    }
}

fn csv_proton(t: &Table, out: &mut Parsed) {
    let c = |n: &str| t.col(&[n]);
    let (typ, name, url, email, user, pass, notes, totp, created, modified, vault) =
        (c("type"), c("name"), c("url"), c("email"), c("username"), c("password"), c("note"), c("totp"), c("createtime"), c("modifytime"), c("vault"));
    for row in &t.rows {
        let f = Some(get(row, vault).trim().to_string()).filter(|g| !g.is_empty());
        let mut it = match get(row, typ) {
            "note" => note(get(row, name), get(row, notes)),
            _ => {
                let u = if get(row, user).is_empty() { get(row, email) } else { get(row, user) };
                login(get(row, name), get(row, url), u, get(row, pass), get(row, notes), get(row, totp))
            }
        };
        it.created_at = parse_ms(get(row, created));
        it.updated_at = parse_ms(get(row, modified)).max(it.created_at);
        if it.kind == ItemKind::Login {
            push_login(out, it, f, get(row, name));
        } else {
            out.candidates.push(Candidate { item: it, folder: f, issue: None });
        }
    }
}

fn csv_keepass(t: &Table, out: &mut Parsed) {
    let c = |n: &[&str]| t.col(n);
    let (group, title, user, pass, url, notes, totp) = (
        c(&["group", "grupo"]),
        c(&["title", "account", "título"]),
        c(&["username", "login name", "user name"]),
        c(&["password", "senha"]),
        c(&["url", "web site"]),
        c(&["notes", "comments"]),
        c(&["totp", "otp"]),
    );
    for row in &t.rows {
        let f = Some(get(row, group).trim().trim_start_matches("Root/").to_string()).filter(|g| !g.is_empty() && g != "Root");
        let it = login(get(row, title), get(row, url), get(row, user), get(row, pass), get(row, notes), get(row, totp));
        push_login(out, it, f, get(row, title));
    }
}

/// Best-effort mapping for any CSV with recognizable headers (PT or EN).
fn csv_generic(t: &Table, out: &mut Parsed) {
    let c = |n: &[&str]| t.col(n);
    let title = c(&["name", "title", "nome", "título", "titulo", "site", "serviço", "servico", "account"]);
    let url = c(&["url", "website", "web site", "login_uri", "endereço", "endereco", "link"]);
    let user = c(&["username", "user", "login", "email", "e-mail", "usuário", "usuario", "login_username"]);
    let pass = c(&["password", "senha", "pass", "login_password"]);
    let notes = c(&["notes", "note", "notas", "observações", "observacoes", "comentários", "extra"]);
    let totp = c(&["totp", "otp", "2fa", "login_totp"]);
    if pass.is_none() && user.is_none() {
        out.skipped.push(("Arquivo".into(), "Não encontramos colunas de usuário ou senha.".into()));
        return;
    }
    for row in &t.rows {
        let it = login(get(row, title), get(row, url), get(row, user), get(row, pass), get(row, notes), get(row, totp));
        push_login(out, it, None, get(row, title));
    }
}

fn push_login(out: &mut Parsed, it: Item, folder: Option<String>, label: &str) {
    let has_user = it.details.value("username").is_some();
    let has_pass = it.details.value("password").is_some();
    if !has_user && !has_pass && it.overview.urls.is_empty() && it.details.notes.is_empty() {
        out.skipped.push((if label.is_empty() { "Linha sem nome".into() } else { label.to_string() }, "Linha vazia".into()));
        return;
    }
    let issue = (!has_pass).then(|| "Sem senha".to_string());
    out.candidates.push(Candidate { item: it, folder, issue });
}

fn detect_csv(t: &Table) -> Source {
    let has = |h: &str| t.headers.iter().any(|x| x == h);
    if has("login_uri") && has("login_password") {
        Source::Bitwarden
    } else if has("grouping") && has("extra") {
        Source::LastPass
    } else if has("httprealm") || has("formactionorigin") || has("timepasswordchanged") {
        Source::Firefox
    } else if has("vault") && has("createtime") {
        Source::ProtonPass
    } else if has("group") && has("title") && (has("username") || has("user name")) {
        Source::KeePass
    } else if has("name") && has("url") && has("username") && has("password") && t.headers.len() <= 5 {
        Source::Chrome
    } else {
        Source::GenericCsv
    }
}

// ---- Bitwarden JSON --------------------------------------------------------------------

fn str_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn json_bitwarden(v: &Value, out: &mut Parsed) -> Result<()> {
    if v.get("encrypted").and_then(Value::as_bool) == Some(true) {
        return Err(CoreError::Invalid(
            "este arquivo do Bitwarden está protegido por senha. Exporte de novo escolhendo “.json” sem criptografia.".into(),
        ));
    }
    let folders: HashMap<String, String> = v
        .get("folders")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|f| (str_of(f, "id"), str_of(f, "name"))).collect())
        .unwrap_or_default();
    let items = v.get("items").and_then(Value::as_array).ok_or_else(|| CoreError::Invalid("JSON do Bitwarden sem itens".into()))?;
    for i in items {
        let name = str_of(i, "name");
        let notes = str_of(i, "notes");
        let folder = i.get("folderId").and_then(Value::as_str).and_then(|id| folders.get(id).cloned());
        let mut it = match i.get("type").and_then(Value::as_i64).unwrap_or(1) {
            1 => {
                let l = i.get("login").cloned().unwrap_or(Value::Null);
                let uris: Vec<String> = l
                    .get("uris")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|u| str_of(u, "uri")).filter(|u| !u.is_empty()).collect())
                    .unwrap_or_default();
                let mut it = login(&name, &uris.join("\n"), &str_of(&l, "username"), &str_of(&l, "password"), &notes, &str_of(&l, "totp"));
                if let Some(creds) = l.get("fido2Credentials").and_then(Value::as_array) {
                    for c in creds {
                        it.details.passkeys.push(Passkey {
                            credential_id: str_of(c, "credentialId"),
                            rp_id: str_of(c, "rpId"),
                            user_name: str_of(c, "userName"),
                            user_handle: str_of(c, "userHandle"),
                            private_key: SecretString::new(str_of(c, "keyValue")),
                            created_at: 0,
                        });
                    }
                }
                it
            }
            2 => note(&name, &notes),
            3 => {
                let c = i.get("card").cloned().unwrap_or(Value::Null);
                let mut it = new_item(ItemKind::Card, &name);
                let exp = match (str_of(&c, "expMonth"), str_of(&c, "expYear")) {
                    (m, y) if !m.is_empty() && !y.is_empty() => format!("{:0>2}/{}", m, &y[y.len().saturating_sub(2)..]),
                    _ => String::new(),
                };
                it.details.fields = [
                    field("cardholder", FieldType::Text, &str_of(&c, "cardholderName")),
                    field("number", FieldType::CardNumber, &str_of(&c, "number")),
                    field("expiry", FieldType::MonthYear, &exp),
                    field("cvv", FieldType::Pin, &str_of(&c, "code")),
                ]
                .into_iter()
                .flatten()
                .collect();
                it.details.notes = s(&notes);
                it
            }
            4 => {
                let d = i.get("identity").cloned().unwrap_or(Value::Null);
                let mut it = new_item(ItemKind::Identity, &name);
                let full = [str_of(&d, "firstName"), str_of(&d, "middleName"), str_of(&d, "lastName")]
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let sec = |f: Option<Field>, sec: &str| f.map(|mut x| {
                    x.section = Some(sec.into());
                    x
                });
                it.details.fields = [
                    sec(field("fullName", FieldType::Text, &full), "personal"),
                    sec(field("email", FieldType::Email, &str_of(&d, "email")), "personal"),
                    sec(field("phone", FieldType::Phone, &str_of(&d, "phone")), "personal"),
                    sec(field("cpf", FieldType::Cpf, &str_of(&d, "ssn")), "personal"),
                    sec(field("street", FieldType::Text, &str_of(&d, "address1")), "address"),
                    sec(field("complement", FieldType::Text, &str_of(&d, "address2")), "address"),
                    sec(field("city", FieldType::Text, &str_of(&d, "city")), "address"),
                    sec(field("state", FieldType::Select, &str_of(&d, "state")), "address"),
                    sec(field("cep", FieldType::Cep, &str_of(&d, "postalCode")), "address"),
                ]
                .into_iter()
                .flatten()
                .collect();
                it.details.notes = s(&notes);
                it
            }
            5 => {
                let k = i.get("sshKey").cloned().unwrap_or(Value::Null);
                let mut it = new_item(ItemKind::SshKey, &name);
                it.details.fields = [
                    field("privateKey", FieldType::SshPrivateKey, &str_of(&k, "privateKey")),
                    field("publicKey", FieldType::Multiline, &str_of(&k, "publicKey")),
                    field("fingerprint", FieldType::Text, &str_of(&k, "keyFingerprint")),
                ]
                .into_iter()
                .flatten()
                .collect();
                it.details.notes = s(&notes);
                it
            }
            other => {
                out.skipped.push((name.clone(), format!("Tipo {other} do Bitwarden não suportado")));
                continue;
            }
        };
        if let Some(fs) = i.get("fields").and_then(Value::as_array) {
            for f in fs {
                let kind = if f.get("type").and_then(Value::as_i64) == Some(1) { FieldType::Concealed } else { FieldType::Text };
                it.details.fields.extend(custom(&str_of(f, "name"), kind, &str_of(f, "value")));
            }
        }
        if let Some(h) = i.get("passwordHistory").and_then(Value::as_array) {
            for e in h {
                it.details.password_history.push(PasswordHistoryEntry { value: SecretString::new(str_of(e, "password")), replaced_at: 0 });
            }
        }
        it.overview.favorite = i.get("favorite").and_then(Value::as_bool).unwrap_or(false);
        let issue = (it.kind == ItemKind::Login && it.details.value("password").is_none()).then(|| "Sem senha".to_string());
        out.candidates.push(Candidate { item: it, folder, issue });
    }
    Ok(())
}

// ---- 1Password 1PUX --------------------------------------------------------------------

fn onepux(data: &[u8], out: &mut Parsed) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| CoreError::Invalid("arquivo .1pux inválido".into()))?;
    let mut entry = zip.by_name("export.data").map_err(|_| CoreError::Invalid("o .1pux não tem export.data".into()))?;
    let mut raw = Zeroizing::new(String::new());
    entry.read_to_string(&mut raw).map_err(|_| CoreError::Invalid("export.data ilegível".into()))?;
    let v: Value = serde_json::from_str(&raw)?;
    let accounts = v.get("accounts").and_then(Value::as_array).cloned().unwrap_or_default();
    for acct in accounts {
        for vault in acct.get("vaults").and_then(Value::as_array).cloned().unwrap_or_default() {
            let vault_name = vault.pointer("/attrs/name").and_then(Value::as_str).map(String::from);
            for i in vault.get("items").and_then(Value::as_array).cloned().unwrap_or_default() {
                let ov = i.get("overview").cloned().unwrap_or(Value::Null);
                let det = i.get("details").cloned().unwrap_or(Value::Null);
                let title = str_of(&ov, "title");
                let cat = str_of(&i, "categoryUuid");
                let mut user = String::new();
                let mut pass = String::new();
                for lf in det.get("loginFields").and_then(Value::as_array).cloned().unwrap_or_default() {
                    match str_of(&lf, "designation").as_str() {
                        "username" => user = str_of(&lf, "value"),
                        "password" => pass = str_of(&lf, "value"),
                        _ => {}
                    }
                }
                if pass.is_empty() {
                    pass = str_of(&det, "password");
                }
                let notes = str_of(&det, "notesPlain");
                let mut it = match cat.as_str() {
                    "003" => note(&title, &notes),
                    "005" => {
                        let mut it = new_item(ItemKind::Password, &title);
                        it.details.fields.extend(field("password", FieldType::Concealed, &pass));
                        it.details.notes = s(&notes);
                        it
                    }
                    _ => login(&title, &str_of(&ov, "url"), &user, &pass, &notes, ""),
                };
                // Extra section fields (TOTP, custom).
                for sec in det.get("sections").and_then(Value::as_array).cloned().unwrap_or_default() {
                    for f in sec.get("fields").and_then(Value::as_array).cloned().unwrap_or_default() {
                        let label = str_of(&f, "title");
                        let val = f.get("value").cloned().unwrap_or(Value::Null);
                        if let Some(totp) = val.get("totp").and_then(Value::as_str) {
                            if it.kind == ItemKind::Login && it.details.field("totp").is_none() {
                                it.details.fields.extend(field("totp", FieldType::Totp, totp));
                            } else {
                                it.details.fields.extend(custom(&label, FieldType::Totp, totp));
                            }
                        } else if let Some(c) = val.get("concealed").and_then(Value::as_str) {
                            it.details.fields.extend(custom(&label, FieldType::Concealed, c));
                        } else if let Some(t) = val.get("string").and_then(Value::as_str) {
                            it.details.fields.extend(custom(&label, FieldType::Text, t));
                        } else if let Some(t) = val.get("email").and_then(|e| e.get("email_address")).and_then(Value::as_str) {
                            it.details.fields.extend(custom(&label, FieldType::Email, t));
                        }
                    }
                }
                if let Some(tags) = ov.get("tags").and_then(Value::as_array) {
                    it.overview.tags = tags.iter().filter_map(Value::as_str).map(String::from).collect();
                }
                it.overview.favorite = i.get("favIndex").and_then(Value::as_i64).unwrap_or(0) > 0;
                it.created_at = i.get("createdAt").and_then(Value::as_i64).unwrap_or(0) * 1000;
                it.updated_at = i.get("updatedAt").and_then(Value::as_i64).unwrap_or(0) * 1000;
                if i.get("state").and_then(Value::as_str) == Some("archived") {
                    it.overview.archived = true;
                }
                out.candidates.push(Candidate { item: it, folder: vault_name.clone(), issue: None });
            }
        }
    }
    Ok(())
}

// ---- entry point -------------------------------------------------------------------------

pub fn parse(data: &[u8], hint: Source, file_name: &str) -> Result<Parsed> {
    let lower = file_name.to_lowercase();
    let mut out = Parsed::default();
    if data.starts_with(b"PK") || lower.ends_with(".1pux") {
        onepux(data, &mut out)?;
        out.source = Some(Source::OnePassword);
    } else if data.starts_with(crate::export::MAGIC) {
        return Err(CoreError::Invalid("arquivo do Mocó cifrado: use “Importar backup do Mocó”".into()));
    } else if lower.ends_with(".json") || data.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        let text = Zeroizing::new(String::from_utf8_lossy(data).trim_start_matches('\u{feff}').to_string());
        let v: Value = serde_json::from_str(&text).map_err(|_| CoreError::Invalid("JSON inválido".into()))?;
        json_bitwarden(&v, &mut out)?;
        out.source = Some(Source::Bitwarden);
    } else {
        let t = read_csv(data)?;
        let src = if hint == Source::Auto || hint == Source::OnePassword { detect_csv(&t) } else { hint };
        match src {
            Source::Chrome => csv_chrome(&t, &mut out),
            Source::Firefox => csv_firefox(&t, &mut out),
            Source::LastPass => csv_lastpass(&t, &mut out),
            Source::Bitwarden => csv_bitwarden(&t, &mut out),
            Source::ProtonPass => csv_proton(&t, &mut out),
            Source::KeePass => csv_keepass(&t, &mut out),
            _ => csv_generic(&t, &mut out),
        }
        out.source = Some(src);
    }
    let now = now_ms();
    for c in out.candidates.iter_mut() {
        if c.item.created_at == 0 {
            c.item.created_at = now;
        }
        if c.item.updated_at == 0 {
            c.item.updated_at = c.item.created_at;
        }
    }
    Ok(out)
}

/// Identity used to spot duplicates: kind + host + username (+ title when no host).
pub fn dedupe_key(item: &Item) -> String {
    let host = item.overview.urls.first().map(|u| url_host(u)).unwrap_or_default();
    let user = item.details.value("username").unwrap_or("").to_lowercase();
    let base = if host.is_empty() { item.overview.title.to_lowercase() } else { host };
    format!("{:?}|{}|{}", item.kind, base, user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_csv() {
        let csv = b"name,url,username,password,note\nNubank,https://app.nubank.com.br/,ana,senha1,\n,https://github.com/login,ana,senha2,obs\n";
        let p = parse(csv, Source::Auto, "Chrome Passwords.csv").unwrap();
        assert_eq!(p.source, Some(Source::Chrome));
        assert_eq!(p.candidates.len(), 2);
        assert_eq!(p.candidates[1].item.overview.title, "Github");
        assert_eq!(p.candidates[1].item.details.notes.expose(), "obs");
    }

    #[test]
    fn semicolon_generic_csv_in_portuguese() {
        let csv = "Nome;Endereço;Usuário;Senha\nGmail;mail.google.com;ana@x.com;abc123\n".as_bytes();
        let p = parse(csv, Source::Auto, "planilha.csv").unwrap();
        assert_eq!(p.source, Some(Source::GenericCsv));
        assert_eq!(p.candidates[0].item.details.value("password"), Some("abc123"));
    }

    #[test]
    fn lastpass_notes_and_folders() {
        let csv = b"url,username,password,totp,extra,name,grouping,fav\nhttp://sn,,,,texto secreto,Cofre,Casa,1\nhttps://x.com,u,p,,,X,Trabalho,0\n";
        let p = parse(csv, Source::Auto, "lastpass.csv").unwrap();
        assert_eq!(p.source, Some(Source::LastPass));
        assert_eq!(p.candidates[0].item.kind, ItemKind::Note);
        assert!(p.candidates[0].item.overview.favorite);
        assert_eq!(p.candidates[1].folder.as_deref(), Some("Trabalho"));
    }

    #[test]
    fn bitwarden_json() {
        let json = r#"{"encrypted":false,"folders":[{"id":"f1","name":"Banco"}],"items":[
          {"type":1,"name":"Itaú","folderId":"f1","favorite":true,"notes":null,
           "login":{"username":"ana","password":"p","totp":"JBSWY3DPEHPK3PXP","uris":[{"uri":"https://itau.com.br"}]},
           "fields":[{"name":"Agência","value":"0001","type":0}]},
          {"type":3,"name":"Cartão","card":{"cardholderName":"ANA","number":"4111111111111111","expMonth":"8","expYear":"2031","code":"123"}},
          {"type":2,"name":"Nota","notes":"oi"}]}"#;
        let p = parse(json.as_bytes(), Source::Auto, "bitwarden.json").unwrap();
        assert_eq!(p.candidates.len(), 3);
        let itau = &p.candidates[0];
        assert_eq!(itau.folder.as_deref(), Some("Banco"));
        assert_eq!(itau.item.details.value("totp"), Some("JBSWY3DPEHPK3PXP"));
        assert!(itau.item.details.fields.iter().any(|f| f.label == "Agência"));
        assert_eq!(p.candidates[1].item.details.value("expiry"), Some("08/31"));
        assert!(parse(br#"{"encrypted":true}"#, Source::Auto, "b.json").is_err());
    }

    #[test]
    fn dedupe_keys() {
        let a = login("GitHub", "https://github.com/login", "Ana", "x", "", "");
        let b = login("Github (pessoal)", "https://www.github.com", "ana", "y", "", "");
        assert_eq!(dedupe_key(&a), dedupe_key(&b));
    }
}
