//! Shapes sent to the UI. Concealed values are withheld unless explicitly requested.

use moco_core::model::{FieldType, Item, ItemKind, Section, Timestamp};
use moco_core::strength;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    /// `None` for concealed fields: fetch with `field_reveal` when the user asks.
    pub value: Option<String>,
    pub has_value: bool,
    pub section: Option<String>,
    /// 0–4, only for concealed password-like fields.
    pub strength: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryView {
    pub replaced_at: Timestamp,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyView {
    pub rp_id: String,
    pub user_name: String,
    pub created_at: Timestamp,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentView {
    pub id: Uuid,
    pub name: String,
    pub size: u64,
    pub mime: String,
    pub added_at: Timestamp,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
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
    pub fields: Vec<FieldView>,
    pub sections: Vec<Section>,
    pub notes: String,
    pub password_history: Vec<HistoryEntryView>,
    pub attachments: Vec<AttachmentView>,
    pub passkeys: Vec<PasskeyView>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub revision: u64,
}

impl From<&Item> for ItemView {
    fn from(item: &Item) -> Self {
        let d = &item.details;
        let o = &item.overview;
        let fields = d
            .fields
            .iter()
            .map(|f| {
                let concealed = f.kind.is_concealed() || f.kind == FieldType::CardNumber;
                let strength = (f.kind == FieldType::Concealed && !f.value.is_empty())
                    .then(|| strength::estimate(f.value.expose(), &[o.title.as_str()]).score);
                FieldView {
                    id: f.id.clone(),
                    label: f.label.clone(),
                    kind: f.kind,
                    value: if concealed { None } else { Some(f.value.expose().to_string()) },
                    has_value: !f.value.is_empty(),
                    section: f.section.clone(),
                    strength,
                }
            })
            .collect();
        ItemView {
            id: item.id,
            vault_id: item.vault_id,
            kind: item.kind,
            title: o.title.clone(),
            subtitle: o.subtitle.clone(),
            urls: o.urls.clone(),
            tags: o.tags.clone(),
            favorite: o.favorite,
            archived: o.archived,
            trashed_at: o.trashed_at,
            icon: o.icon.clone(),
            fields,
            sections: d.sections.clone(),
            notes: d.notes.expose().to_string(),
            password_history: d.password_history.iter().map(|h| HistoryEntryView { replaced_at: h.replaced_at }).collect(),
            attachments: d
                .attachments
                .iter()
                .map(|a| AttachmentView { id: a.id, name: a.name.clone(), size: a.size, mime: a.mime.clone(), added_at: a.added_at })
                .collect(),
            passkeys: d
                .passkeys
                .iter()
                .map(|p| PasskeyView { rp_id: p.rp_id.clone(), user_name: p.user_name.clone(), created_at: p.created_at })
                .collect(),
            created_at: item.created_at,
            updated_at: item.updated_at,
            revision: item.revision,
        }
    }
}
