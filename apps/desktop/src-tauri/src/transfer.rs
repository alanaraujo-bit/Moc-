//! Import and export commands. Files are chosen with native dialogs and read/written by
//! Rust — plaintext exports never pass through the webview.

use moco_core::export::{self, VaultExport};
use moco_core::import::{self, Candidate, Source};
use moco_core::model::{normalize_tags, Item};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::commands::PasswordArgs;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;

#[derive(Default)]
pub struct PendingImport {
    pub file_name: String,
    pub source: Option<Source>,
    pub candidates: Vec<(Candidate, bool)>, // (candidate, is duplicate)
    pub backup_vaults: Vec<VaultExport>,
}

pub type Pending = Mutex<Option<PendingImport>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    pub index: usize,
    pub title: String,
    pub subtitle: String,
    pub kind: moco_core::model::ItemKind,
    pub folder: Option<String>,
    pub issue: Option<String>,
    pub duplicate: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub file_name: String,
    pub source: String,
    pub total: usize,
    pub duplicates: usize,
    pub without_password: usize,
    pub folders: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub rows: Vec<PreviewRow>,
}

fn cloud_synced(p: &Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    ["onedrive", "dropbox", "google drive", "googledrive", "icloud", "meu drive"].iter().any(|k| s.contains(k))
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> impl std::future::Future<Output = AppResult<T>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::internal(e.to_string()))? }
}

fn pick_file(app: &AppHandle, title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
    // Automated QA can't click a native dialog; debug builds accept a path from the env.
    if cfg!(debug_assertions) {
        if let Ok(p) = std::env::var("MOCO_QA_PICK") {
            return Some(PathBuf::from(p));
        }
    }
    let mut d = app.dialog().file().set_title(title);
    for (name, exts) in filters {
        d = d.add_filter(*name, exts);
    }
    d.blocking_pick_file().and_then(|f| f.into_path().ok())
}

fn save_file(app: &AppHandle, title: &str, name: &str, filter: (&str, &[&str])) -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        if let Ok(dir) = std::env::var("MOCO_QA_SAVE_DIR") {
            return Some(PathBuf::from(dir).join(name));
        }
    }
    app.dialog()
        .file()
        .set_title(title)
        .set_file_name(name)
        .add_filter(filter.0, filter.1)
        .blocking_save_file()
        .and_then(|f| f.into_path().ok())
}

fn build_preview(st: &AppState, file_name: String, parsed: import::Parsed) -> AppResult<Preview> {
    let existing: HashSet<String> = st.with_account(|a| Ok(a.all_items()?))?.iter().map(import::dedupe_key).collect();
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for c in parsed.candidates {
        let key = import::dedupe_key(&c.item);
        let dup = existing.contains(&key) || !seen.insert(key);
        candidates.push((c, dup));
    }
    let mut folders: Vec<String> = candidates.iter().filter_map(|(c, _)| c.folder.clone()).collect::<HashSet<_>>().into_iter().collect();
    folders.sort();
    let rows = candidates
        .iter()
        .enumerate()
        .map(|(index, (c, dup))| {
            let mut it = c.item.clone();
            it.refresh_derived();
            PreviewRow {
                index,
                title: it.overview.title.clone(),
                subtitle: it.overview.subtitle.clone(),
                kind: it.kind,
                folder: c.folder.clone(),
                issue: c.issue.clone(),
                duplicate: *dup,
            }
        })
        .collect();
    let preview = Preview {
        file_name: file_name.clone(),
        source: parsed.source.map(|s| s.label().to_string()).unwrap_or_default(),
        total: candidates.len(),
        duplicates: candidates.iter().filter(|(_, d)| *d).count(),
        without_password: candidates.iter().filter(|(c, _)| c.issue.as_deref() == Some("Sem senha")).count(),
        folders,
        skipped: parsed.skipped,
        rows,
    };
    Ok(preview_store(st, file_name, parsed.source, candidates, vec![], preview))
}

fn preview_store(st: &AppState, file_name: String, source: Option<Source>, candidates: Vec<(Candidate, bool)>, backup_vaults: Vec<VaultExport>, p: Preview) -> Preview {
    if let Ok(mut g) = st.pending_import.lock() {
        *g = Some(PendingImport { file_name, source, candidates, backup_vaults });
    }
    p
}

#[tauri::command]
pub async fn import_pick(app: AppHandle, state: S<'_>, source: Source) -> AppResult<Option<Preview>> {
    let st = state.inner().clone();
    blocking(move || {
        let Some(path) = pick_file(&app, "Escolha o arquivo exportado", &[("Exportações", &["csv", "json", "1pux", "txt"]), ("Todos os arquivos", &["*"])]) else {
            return Ok(None);
        };
        let meta = std::fs::metadata(&path)?;
        if meta.len() > 50 * 1024 * 1024 {
            return Err(AppError::new("too_large", "Arquivo grande demais (mais de 50 MB)."));
        }
        let data = zeroize::Zeroizing::new(std::fs::read(&path)?);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let parsed = import::parse(&data, source, &name)?;
        if parsed.candidates.is_empty() {
            return Err(AppError::new("empty", "Não encontramos nada para importar nesse arquivo. Confira se é a exportação certa."));
        }
        Ok(Some(build_preview(&st, name, parsed)?))
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPickArgs {
    pub password: moco_core::crypto::SecretString,
}

#[tauri::command]
pub async fn import_backup_pick(app: AppHandle, state: S<'_>, args: BackupPickArgs) -> AppResult<Option<Preview>> {
    let st = state.inner().clone();
    blocking(move || {
        let Some(path) = pick_file(&app, "Escolha o backup do Mocó", &[("Backup do Mocó", &["moco"])]) else {
            return Ok(None);
        };
        let data = std::fs::read(&path)?;
        let backup = export::open_backup(&data, args.password.expose()).map_err(|e| match e {
            moco_core::CoreError::BadCredentials => AppError::new("bad_credentials", "Senha do backup incorreta."),
            other => other.into(),
        })?;
        let vault_names: HashMap<Uuid, String> = backup.vaults.iter().map(|v| (v.id, v.name.clone())).collect();
        let parsed = import::Parsed {
            source: Some(Source::Moco),
            candidates: backup
                .items
                .into_iter()
                .map(|item| Candidate { folder: vault_names.get(&item.vault_id).cloned(), item, issue: None })
                .collect(),
            skipped: vec![],
        };
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let preview = build_preview(&st, name, parsed)?;
        if let Ok(mut g) = st.pending_import.lock() {
            if let Some(p) = g.as_mut() {
                p.backup_vaults = backup.vaults;
            }
        }
        Ok(Some(preview))
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitArgs {
    pub vault_id: Uuid,
    pub skip_duplicates: bool,
    /// Turn source folders into tags.
    pub folders_as_tags: bool,
    /// Extra tag added to everything imported (e.g. "importado/bitwarden").
    pub tag: Option<String>,
    /// Indices unchecked by the user in the preview.
    #[serde(default)]
    pub exclude: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub imported: usize,
    pub skipped: usize,
}

#[tauri::command]
pub async fn import_commit(state: S<'_>, args: CommitArgs) -> AppResult<CommitResult> {
    let st = state.inner().clone();
    blocking(move || {
        let pending = st
            .pending_import
            .lock()
            .map_err(|_| AppError::internal("estado"))?
            .take()
            .ok_or_else(|| AppError::new("expired", "Escolha o arquivo de novo."))?;
        let exclude: HashSet<usize> = args.exclude.into_iter().collect();
        let (mut imported, mut skipped) = (0, 0);
        let mut batch: Vec<Item> = Vec::new();
        st.with_account(|a| {
            for (i, (c, dup)) in pending.candidates.iter().cloned().enumerate() {
                if exclude.contains(&i) || (dup && args.skip_duplicates) {
                    skipped += 1;
                    continue;
                }
                let mut item: Item = c.item;
                if args.folders_as_tags {
                    if let Some(f) = c.folder {
                        item.overview.tags.push(f.replace('\\', "/"));
                    }
                }
                if let Some(t) = &args.tag {
                    item.overview.tags.push(t.clone());
                }
                item.overview.tags = normalize_tags(&item.overview.tags);
                batch.push(item);
            }
            imported = a.import_items(args.vault_id, std::mem::take(&mut batch))?;
            let src = pending.source.map(|s| s.label()).unwrap_or("?");
            let _ = a.store().log_event("import", Some(format!("{imported} itens · {src} · {}", pending.file_name).as_bytes()));
            Ok(())
        })?;
        Ok(CommitResult { imported, skipped })
    })
    .await
}

#[tauri::command]
pub async fn import_cancel(state: S<'_>) -> AppResult<()> {
    if let Ok(mut g) = state.pending_import.lock() {
        *g = None;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub count: usize,
    /// The chosen folder syncs to a cloud drive.
    pub cloud_synced: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupArgs {
    pub master_password: moco_core::crypto::SecretString,
    pub export_password: moco_core::crypto::SecretString,
}

fn today() -> String {
    let secs = moco_core::model::now_ms() / 1000;
    let days = secs / 86_400;
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

#[tauri::command]
pub async fn export_backup(app: AppHandle, state: S<'_>, args: BackupArgs) -> AppResult<Option<ExportResult>> {
    let st = state.inner().clone();
    let version = app.package_info().version.to_string();
    blocking(move || {
        let sk = crate::commands::device_secret_key(&st)?;
        st.with_account(|a| Ok(a.verify_password(args.master_password.expose(), &sk)?))?;
        let (vaults, items) = st.with_account(|a| {
            let vaults = a.vaults()?.into_iter().map(|v| VaultExport { id: v.id, name: v.attrs.name, color: v.attrs.color }).collect();
            Ok((vaults, a.all_items()?))
        })?;
        let count = items.len();
        let sealed = export::seal_backup(&export::new_backup(&version, vaults, items), args.export_password.expose())?;
        let Some(path) = save_file(&app, "Salvar backup do Mocó", &format!("Mocó backup {}.moco", today()), ("Backup do Mocó", &["moco"])) else {
            return Ok(None);
        };
        std::fs::write(&path, sealed)?;
        st.with_account(|a| Ok(a.store().log_event("export.backup", None)?))?;
        Ok(Some(ExportResult { cloud_synced: cloud_synced(&path), path: path.to_string_lossy().to_string(), count }))
    })
    .await
}

#[tauri::command]
pub async fn export_csv(app: AppHandle, state: S<'_>, args: PasswordArgs) -> AppResult<Option<ExportResult>> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = crate::commands::device_secret_key(&st)?;
        st.with_account(|a| Ok(a.verify_password(args.password.expose(), &sk)?))?;
        let items = st.with_account(|a| Ok(a.all_items()?))?;
        let csv = export::logins_csv(&items);
        let count = csv.lines().count().saturating_sub(1);
        let Some(path) = save_file(&app, "Salvar logins em CSV (sem proteção)", &format!("Mocó logins {}.csv", today()), ("CSV", &["csv"])) else {
            return Ok(None);
        };
        std::fs::write(&path, csv.as_bytes())?;
        st.with_account(|a| Ok(a.store().log_event("export.csv", None)?))?;
        Ok(Some(ExportResult { cloud_synced: cloud_synced(&path), path: path.to_string_lossy().to_string(), count }))
    })
    .await
}
