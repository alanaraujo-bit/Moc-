//! End-to-end tests against a real Postgres: real routes, real clients (`moco-core`).
//! Set `TEST_DATABASE_URL` to run them; without it they are skipped.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use moco_core::account::sharing::{KeyCheck, Member, SharedEntry};
use moco_core::account::sync::{PullResponse, Push, PushRequest, PushResponse};
use moco_core::account::{Account, VaultAttrs};
use moco_core::crypto::identity::PublicIdentity;
use moco_core::crypto::kdf::KdfParams;
use moco_core::crypto::share::{Role, ShareGrant};
use moco_core::crypto::{SecretKey, SecretString};
use moco_core::model::{Field, FieldType, ItemInput, ItemKind};
use moco_core::store::Store;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{ratelimit, router, secrets, AppState};

async fn app() -> Option<Router> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let db = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&url).await.expect("test database");
    sqlx::migrate!("./migrations").run(&db).await.expect("migrations");
    Some(router(Arc::new(AppState { db, secrets: secrets::ServerSecrets::for_tests(), limits: ratelimit::Limiter::default() })))
}

async fn call(app: &Router, method: &str, path: &str, token: Option<&str>, body: Option<Vec<u8>>, json: bool) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if json {
        req = req.header("content-type", "application/json");
    }
    let resp = app.clone().oneshot(req.body(body.map(Body::from).unwrap_or_else(Body::empty)).unwrap()).await.unwrap();
    let status = resp.status();
    (status, resp.into_body().collect().await.unwrap().to_bytes().to_vec())
}

async fn get_json<T: serde::de::DeserializeOwned>(app: &Router, path: &str, token: &str) -> (StatusCode, Option<T>) {
    let (s, b) = call(app, "GET", path, Some(token), None, false).await;
    (s, serde_json::from_slice(&b).ok())
}

async fn send_json<T: serde::Serialize>(app: &Router, method: &str, path: &str, token: &str, body: &T) -> (StatusCode, Vec<u8>) {
    call(app, method, path, Some(token), Some(serde_json::to_vec(body).unwrap()), true).await
}

struct Person {
    acct: Account,
    token: String,
    email: String,
    id: Uuid,
}

async fn person(app: &Router, name: &str) -> Person {
    let mut acct = Account::new(Store::open_in_memory().unwrap());
    acct.upgrade_kdf_on_unlock = false;
    let sk = SecretKey::generate();
    acct.create_with_params(
        "correct horse battery",
        &sk,
        VaultAttrs { name: "Pessoal".into(), description: String::new(), icon: String::new(), color: String::new() },
        KdfParams::fast_for_tests(),
    )
    .unwrap();
    let id = acct.account_id().unwrap();
    let email = format!("{name}-{}@teste.moco", Uuid::new_v4().simple());
    let record = acct.record_json().unwrap();
    let rec: Value = serde_json::from_str(&record).unwrap();
    let body = json!({
        "email": email, "accountId": id, "record": record, "authPublicKey": rec["authPublicKey"],
        "device": { "id": Uuid::new_v4(), "name": "teste", "platform": "windows" },
    });
    let (s, b) = call(app, "POST", "/v1/register", None, Some(serde_json::to_vec(&body).unwrap()), true).await;
    assert_eq!(s, StatusCode::CREATED, "{}", String::from_utf8_lossy(&b));
    let token = serde_json::from_slice::<Value>(&b).unwrap()["token"].as_str().unwrap().to_string();
    Person { acct, token, email, id }
}

fn login(title: &str, pass: &str) -> ItemInput {
    ItemInput {
        kind: ItemKind::Login,
        title: title.into(),
        urls: vec![],
        tags: vec![],
        favorite: false,
        icon: None,
        fields: vec![Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new(pass), section: None }],
        sections: vec![],
        notes: SecretString::default(),
    }
}

fn family(p: &mut Person) -> Uuid {
    p.acct.create_vault(VaultAttrs { name: "Família".into(), description: String::new(), icon: String::new(), color: String::new() }).unwrap().id
}

fn titles(a: &Account, vault: Uuid) -> Vec<String> {
    let mut t: Vec<String> = a.summaries().unwrap().into_iter().filter(|s| s.vault_id == vault).map(|s| s.title).collect();
    t.sort();
    t
}

fn password(a: &Account, id: Uuid) -> String {
    a.item(id).unwrap().details.fields[0].value.expose().to_string()
}

/// Own-account sync over HTTP (what the desktop loop does).
async fn sync_own(app: &Router, p: &mut Person) {
    for _ in 0..3 {
        let (upto, req) = p.acct.sync_collect().unwrap();
        if !req.is_empty() {
            let (s, b) = send_json(app, "POST", "/v1/sync/push", &p.token, &req).await;
            assert_eq!(s, StatusCode::OK, "{}", String::from_utf8_lossy(&b));
            p.acct.sync_apply_push(upto, &serde_json::from_slice(&b).unwrap()).unwrap();
        }
        let path = format!("/v1/sync/pull?since={}&recordVersion={}", p.acct.sync_cursor().unwrap(), p.acct.record_version().unwrap());
        let (s, pull) = get_json::<PullResponse>(app, &path, &p.token).await;
        assert_eq!(s, StatusCode::OK);
        p.acct.sync_apply_pull(&pull.unwrap()).unwrap();
        if p.acct.sync_collect().unwrap().1.is_empty() {
            break;
        }
    }
}

/// Own sync plus every shared vault. Returns the statuses of shared pushes.
async fn sync_all(app: &Router, p: &mut Person) -> Vec<StatusCode> {
    sync_own(app, p).await;
    let (s, entries) = get_json::<Vec<SharedEntry>>(app, "/v1/shared", &p.token).await;
    assert_eq!(s, StatusCode::OK);
    let entries = entries.unwrap();
    for (vault, _) in p.acct.shared_vault_ids().unwrap() {
        if !entries.iter().any(|e| e.grant.vault_id == vault) {
            p.acct.forget_shared_vault(vault).unwrap();
        }
    }
    let mut pushes = Vec::new();
    for e in entries {
        let (owner, vault) = (e.owner_id, e.grant.vault_id);
        if p.acct.shared_vault_ids().unwrap().iter().any(|(v, _)| *v == vault) {
            let (upto, req) = p.acct.shared_collect(vault).unwrap();
            if !req.is_empty() {
                let (s, b) = send_json(app, "POST", &format!("/v1/shared/{owner}/{vault}/push"), &p.token, &req).await;
                pushes.push(s);
                if s == StatusCode::OK {
                    p.acct.sync_apply_push(upto, &serde_json::from_slice::<PushResponse>(&b).unwrap()).unwrap();
                }
            }
        }
        let since = p.acct.shared_cursor(vault).unwrap();
        let (s, b) = call(app, "GET", &format!("/v1/shared/{owner}/{vault}/pull?since={since}"), Some(&p.token), None, false).await;
        assert_eq!(s, StatusCode::OK);
        let raw: Value = serde_json::from_slice(&b).unwrap();
        assert!(raw["record"].is_null(), "a shared pull must never carry the owner's record");
        p.acct.sync_shared_pull(&e, &serde_json::from_slice(&b).unwrap()).unwrap();
    }
    pushes
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Found {
    account_id: Uuid,
    identity: PublicIdentity,
}

/// Owner looks the member up by email, pins the key, signs and uploads the grant.
async fn share(app: &Router, owner: &Person, vault: Uuid, member: &Person, role: Role) -> ShareGrant {
    let (s, found) = get_json::<Found>(app, &format!("/v1/people?email={}", member.email), &owner.token).await;
    assert_eq!(s, StatusCode::OK);
    let found = found.unwrap();
    assert_eq!(found.account_id, member.id);
    assert_ne!(owner.acct.check_contact(found.account_id, &member.email, &found.identity).unwrap(), KeyCheck::Changed);
    let g = owner.acct.grant_vault(vault, found.account_id, &found.identity, role).unwrap();
    let (s, b) = send_json(app, "PUT", &format!("/v1/vaults/{vault}/members/{}", member.id), &owner.token, &g).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{}", String::from_utf8_lossy(&b));
    g
}

#[tokio::test]
async fn editor_shares_both_ways_and_nothing_leaks() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let mut beto = person(&app, "beto").await;
    let fam = family(&mut ana);
    let netflix = ana.acct.create_item(fam, login("Netflix", "n1")).unwrap();
    sync_own(&app, &mut ana).await;

    let (s, _) = call(&app, "GET", "/v1/people?email=ninguem-aqui@teste.moco", Some(&ana.token), None, false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    share(&app, &ana, fam, &beto, Role::Editor).await;
    let (_, members) = get_json::<Vec<Member>>(&app, &format!("/v1/vaults/{fam}/members"), &ana.token).await;
    let members = members.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!((members[0].account_id, members[0].role, members[0].key_gen), (beto.id, Role::Editor, 1));
    // Someone else can't list Ana's members.
    let (_, other) = get_json::<Vec<Member>>(&app, &format!("/v1/vaults/{fam}/members"), &beto.token).await;
    assert!(other.unwrap().is_empty());

    sync_all(&app, &mut beto).await;
    assert_eq!(password(&beto.acct, netflix.id), "n1");
    // A second round still gets the vault row (not only rows above the cursor).
    sync_all(&app, &mut beto).await;
    assert_eq!(titles(&beto.acct, fam), vec!["Netflix"]);

    beto.acct.update_item(netflix.id, login("Netflix", "n2")).unwrap();
    beto.acct.create_item(fam, login("Wi-Fi casa", "w")).unwrap();
    assert_eq!(sync_all(&app, &mut beto).await, vec![StatusCode::OK]);
    // Nothing of the shared vault landed in Beto's own account.
    let (_, own) = get_json::<PullResponse>(&app, "/v1/sync/pull?since=0", &beto.token).await;
    assert!(own.unwrap().items.iter().all(|i| i.vault_id != fam));

    sync_own(&app, &mut ana).await;
    assert_eq!(password(&ana.acct, netflix.id), "n2");
    assert_eq!(titles(&ana.acct, fam), vec!["Netflix", "Wi-Fi casa"]);

    // A member can't pull or push vaults they weren't given, even the owner's others.
    let personal = ana.acct.vaults().unwrap().into_iter().find(|v| v.attrs.name == "Pessoal").unwrap().id;
    let (s, _) = call(&app, "GET", &format!("/v1/shared/{}/{personal}/pull?since=0", ana.id), Some(&beto.token), None, false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Beto leaves.
    let (s, _) = call(&app, "DELETE", &format!("/v1/vaults/{fam}/members/{}", beto.id), Some(&beto.token), None, false).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    sync_all(&app, &mut beto).await;
    assert!(titles(&beto.acct, fam).is_empty());
    let (_, members) = get_json::<Vec<Member>>(&app, &format!("/v1/vaults/{fam}/members"), &ana.token).await;
    assert!(members.unwrap().is_empty());
}

#[tokio::test]
async fn reader_and_bad_grants_are_refused() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let mut caio = person(&app, "caio").await;
    let fam = family(&mut ana);
    let item = ana.acct.create_item(fam, login("Banco", "b")).unwrap();
    sync_own(&app, &mut ana).await;
    let g = share(&app, &ana, fam, &caio, Role::Reader).await;
    sync_all(&app, &mut caio).await;
    assert_eq!(password(&caio.acct, item.id), "b");

    // A hand-built push from the reader is refused.
    let (_, pull) = get_json::<PullResponse>(&app, &format!("/v1/shared/{}/{fam}/pull?since=0", ana.id), &caio.token).await;
    let mut row = pull.unwrap().items.remove(0);
    let base = row.version;
    row.version += 1;
    let req = PushRequest { vaults: vec![], items: vec![Push { base, row }] };
    let (s, _) = send_json(&app, "POST", &format!("/v1/shared/{}/{fam}/push", ana.id), &caio.token, &req).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "PUT", &format!("/v1/shared/{}/{fam}/attachments/{}?item={}", ana.id, Uuid::new_v4(), item.id), Some(&caio.token), Some(vec![1, 2, 3]), false).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // The server won't store a role the owner didn't sign.
    let mut upgraded = g.clone();
    upgraded.role = Role::Editor;
    let (s, _) = send_json(&app, "PUT", &format!("/v1/vaults/{fam}/members/{}", caio.id), &ana.token, &upgraded).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Nor a grant for someone else's vault, or uploaded by someone who isn't the owner.
    let (s, _) = send_json(&app, "PUT", &format!("/v1/vaults/{fam}/members/{}", caio.id), &caio.token, &g).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, members) = get_json::<Vec<Member>>(&app, &format!("/v1/vaults/{fam}/members"), &ana.token).await;
    assert_eq!(members.unwrap()[0].role, Role::Reader);
}

#[tokio::test]
async fn removal_rotates_and_old_key_writes_are_refused() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let mut beto = person(&app, "beto").await;
    let mut caio = person(&app, "caio").await;
    let fam = family(&mut ana);
    ana.acct.create_item(fam, login("Antigo", "a")).unwrap();
    sync_own(&app, &mut ana).await;
    share(&app, &ana, fam, &beto, Role::Editor).await;
    let caio_old = share(&app, &ana, fam, &caio, Role::Editor).await;
    sync_all(&app, &mut beto).await;
    sync_all(&app, &mut caio).await;

    // Beto edits offline while Ana removes Caio: rotate the key, re-grant who stays.
    beto.acct.create_item(fam, login("Água", "2")).unwrap();
    let (s, _) = call(&app, "DELETE", &format!("/v1/vaults/{fam}/members/{}", caio.id), Some(&ana.token), None, false).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(ana.acct.rotate_vault_key(fam).unwrap(), 2);
    let (_, members) = get_json::<Vec<Member>>(&app, &format!("/v1/vaults/{fam}/members"), &ana.token).await;
    for m in members.unwrap() {
        let g = ana.acct.grant_vault(fam, m.account_id, &m.identity, m.role).unwrap();
        let (s, _) = send_json(&app, "PUT", &format!("/v1/vaults/{fam}/members/{}", m.account_id), &ana.token, &g).await;
        assert_eq!(s, StatusCode::NO_CONTENT);
    }
    ana.acct.create_item(fam, login("Novo segredo", "s")).unwrap();
    sync_own(&app, &mut ana).await;

    // An old signed grant can't be replayed to bring Caio back on the old key.
    let (s, _) = send_json(&app, "PUT", &format!("/v1/vaults/{fam}/members/{}", caio.id), &ana.token, &caio_old).await;
    assert_eq!(s, StatusCode::CONFLICT);
    // Caio is out.
    sync_all(&app, &mut caio).await;
    assert!(titles(&caio.acct, fam).is_empty());
    let (s, _) = call(&app, "GET", &format!("/v1/shared/{}/{fam}/pull?since=0", ana.id), Some(&caio.token), None, false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Beto's first push is sealed with the old key: refused. The pull installs the new
    // key and re-encrypts the pending edit; the next round goes through.
    assert_eq!(sync_all(&app, &mut beto).await, vec![StatusCode::CONFLICT]);
    assert_eq!(sync_all(&app, &mut beto).await, vec![StatusCode::OK]);
    sync_own(&app, &mut ana).await;
    assert_eq!(titles(&ana.acct, fam), vec!["Antigo", "Novo segredo", "Água"]);
    assert_eq!(titles(&beto.acct, fam), vec!["Antigo", "Novo segredo", "Água"]);
}

#[tokio::test]
async fn attachments_stay_inside_the_shared_vault() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let mut beto = person(&app, "beto").await;
    let fam = family(&mut ana);
    let personal = ana.acct.vaults().unwrap().into_iter().find(|v| v.attrs.name == "Pessoal").unwrap().id;
    let private = ana.acct.create_item(personal, login("Privado", "p")).unwrap();
    let shared_item = ana.acct.create_item(fam, login("Compartilhado", "c")).unwrap();
    sync_own(&app, &mut ana).await;
    // Ana's private attachment, uploaded to her own account.
    let secret_att = Uuid::new_v4();
    let (s, _) = call(&app, "PUT", &format!("/v1/attachments/{secret_att}?item={}", private.id), Some(&ana.token), Some(b"segredo".to_vec()), false).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    share(&app, &ana, fam, &beto, Role::Editor).await;
    sync_all(&app, &mut beto).await;

    let base = format!("/v1/shared/{}/{fam}/attachments", ana.id);
    // Beto can't read, overwrite or delete it by reusing its id.
    let (s, _) = call(&app, "GET", &format!("{base}/{secret_att}"), Some(&beto.token), None, false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&app, "PUT", &format!("{base}/{secret_att}?item={}", shared_item.id), Some(&beto.token), Some(b"x".to_vec()), false).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(&app, "PUT", &format!("{base}/{}?item={}", Uuid::new_v4(), private.id), Some(&beto.token), Some(b"x".to_vec()), false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let _ = call(&app, "DELETE", &format!("{base}/{secret_att}"), Some(&beto.token), None, false).await;
    let (s, b) = call(&app, "GET", &format!("/v1/attachments/{secret_att}"), Some(&ana.token), None, false).await;
    assert_eq!((s, b), (StatusCode::OK, b"segredo".to_vec()));

    // Inside the vault it works both ways, counted in Ana's account.
    let att = Uuid::new_v4();
    let (s, _) = call(&app, "PUT", &format!("{base}/{att}?item={}", shared_item.id), Some(&beto.token), Some(b"boleto".to_vec()), false).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, b) = call(&app, "GET", &format!("/v1/attachments/{att}"), Some(&ana.token), None, false).await;
    assert_eq!((s, b), (StatusCode::OK, b"boleto".to_vec()));
    let (s, b) = call(&app, "GET", &format!("{base}/{att}"), Some(&beto.token), None, false).await;
    assert_eq!((s, b), (StatusCode::OK, b"boleto".to_vec()));
    let (s, _) = call(&app, "DELETE", &format!("{base}/{att}"), Some(&beto.token), None, false).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = call(&app, "GET", &format!("/v1/attachments/{att}"), Some(&ana.token), None, false).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deleting_the_vault_ends_the_sharing() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let mut beto = person(&app, "beto").await;
    let fam = family(&mut ana);
    ana.acct.create_item(fam, login("Luz", "1")).unwrap();
    sync_own(&app, &mut ana).await;
    share(&app, &ana, fam, &beto, Role::Editor).await;
    sync_all(&app, &mut beto).await;
    assert_eq!(titles(&beto.acct, fam), vec!["Luz"]);

    let personal = ana.acct.vaults().unwrap().into_iter().find(|v| v.attrs.name == "Pessoal").unwrap().id;
    ana.acct.delete_vault(fam, Some(personal)).unwrap();
    sync_own(&app, &mut ana).await;
    let (_, entries) = get_json::<Vec<SharedEntry>>(&app, "/v1/shared", &beto.token).await;
    assert!(entries.unwrap().is_empty());
    sync_all(&app, &mut beto).await;
    assert!(beto.acct.vaults().unwrap().iter().all(|v| v.id != fam));
}

#[tokio::test]
async fn own_sync_between_two_devices_still_works() {
    let Some(app) = app().await else { return };
    let mut ana = person(&app, "ana").await;
    let personal = ana.acct.vaults().unwrap()[0].id;
    for i in 0..5 {
        ana.acct.create_item(personal, login(&format!("Item {i}"), "x")).unwrap();
    }
    sync_own(&app, &mut ana).await;
    let (_, pull) = get_json::<PullResponse>(&app, "/v1/sync/pull?since=0", &ana.token).await;
    let pull = pull.unwrap();
    assert_eq!(pull.items.len(), 5);
    assert_eq!(pull.vaults.len(), 1);
    let (_, again) = get_json::<PullResponse>(&app, &format!("/v1/sync/pull?since={}", pull.cursor), &ana.token).await;
    assert!(again.unwrap().items.is_empty());
}
