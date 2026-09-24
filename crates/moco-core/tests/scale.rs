//! Scale check (run with: cargo test -p moco-core --release --test scale -- --ignored --nocapture)
use moco_core::account::{Account, VaultAttrs};
use moco_core::crypto::{SecretKey, SecretString};
use moco_core::model::{Field, FieldType, ItemInput, ItemKind};
use moco_core::store::Store;
use std::time::Instant;

#[test]
#[ignore]
fn ten_thousand_items() {
    let dir = std::env::temp_dir().join(format!("moco-scale-{}", std::process::id()));
    let path = dir.join("vault.db");
    let sk = SecretKey::generate();
    let mut a = Account::new(Store::open(&path).unwrap());
    a.create("senha mestra de teste longa", &sk, VaultAttrs { name: "P".into(), description: String::new(), icon: String::new(), color: String::new() }).unwrap();
    let v = a.vaults().unwrap()[0].id;
    let t = Instant::now();
    let items: Vec<_> = (0..10_000)
        .map(|i| ItemInput {
            kind: ItemKind::Login,
            title: format!("Site {i}"),
            urls: vec![format!("https://site{i}.example.com")],
            tags: vec!["trabalho".into()],
            favorite: false,
            icon: None,
            fields: vec![
                Field { id: "username".into(), label: String::new(), kind: FieldType::Text, value: SecretString::new(format!("user{i}")), section: None },
                Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new(format!("pw-{i}-xyz")), section: None },
            ],
            sections: vec![],
            notes: SecretString::default(),
        })
        .collect();
    a.create_items_bulk(v, items).unwrap();
    println!("create 10k (bulk): {:?}", t.elapsed());
    a.lock();
    let t = Instant::now();
    a.unlock("senha mestra de teste longa", &sk).unwrap();
    println!("unlock (Argon2 + decrypt 10k overviews): {:?}", t.elapsed());
    let t = Instant::now();
    let s = a.summaries().unwrap();
    println!("summaries: {} in {:?}", s.len(), t.elapsed());
    let t = Instant::now();
    let _ = a.item(s[5000].id).unwrap();
    println!("open one item: {:?}", t.elapsed());
    let t = Instant::now();
    let all = a.all_items().unwrap();
    let r = moco_core::health::analyze(&all);
    println!("health over {} items: {:?} (weak {})", all.len(), t.elapsed(), r.weak.len());
    drop(a);
    let _ = std::fs::remove_dir_all(dir);
}
