//! Debug-only demonstration data, created through the real account API so QA exercises
//! real encryption and storage. Compiled out of release builds entirely: the command
//! exists there only to return an error.

#[cfg(debug_assertions)]
mod data {
    use moco_core::account::{Account, VaultAttrs};
    use moco_core::crypto::SecretString;
    use moco_core::model::{Field, FieldType, ItemInput, ItemKind};

    fn f(id: &str, kind: FieldType, v: &str) -> Field {
        Field { id: id.into(), label: String::new(), kind, value: SecretString::new(v), section: None }
    }

    fn fs(id: &str, kind: FieldType, v: &str, section: &str) -> Field {
        Field { section: Some(section.into()), ..f(id, kind, v) }
    }

    fn custom(label: &str, kind: FieldType, v: &str) -> Field {
        Field { id: format!("c{}", label.len()), label: label.into(), kind, value: SecretString::new(v), section: None }
    }

    fn item(kind: ItemKind, title: &str, urls: &[&str], tags: &[&str], fav: bool, fields: Vec<Field>, notes: &str) -> ItemInput {
        ItemInput {
            kind,
            title: title.into(),
            urls: urls.iter().map(|s| s.to_string()).collect(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            favorite: fav,
            icon: None,
            fields,
            sections: vec![],
            notes: SecretString::new(notes),
        }
    }

    fn login(title: &str, url: &str, user: &str, pass: &str, tags: &[&str], fav: bool) -> ItemInput {
        item(ItemKind::Login, title, &[url], tags, fav, vec![f("username", FieldType::Text, user), f("password", FieldType::Concealed, pass)], "")
    }

    pub fn seed(acct: &mut Account) -> moco_core::Result<usize> {
        let personal = acct.vaults()?[0].id;
        let work = acct
            .create_vault(VaultAttrs { name: "Trabalho".into(), description: String::new(), icon: "briefcase".into(), color: "slate".into() })?
            .id;
        let family = acct
            .create_vault(VaultAttrs { name: "Família".into(), description: String::new(), icon: "users".into(), color: "moss".into() })?
            .id;

        let mut n = 0;
        let mut add = |vault, input: ItemInput| -> moco_core::Result<()> {
            acct.create_item(vault, input)?;
            n += 1;
            Ok(())
        };

        // Logins — real services, fictional credentials.
        let mut gmail = login("Gmail", "https://mail.google.com", "ana.moura.demo@gmail.com", "Varanda-Cacto-Moqueca8", &["pessoal"], true);
        gmail.fields.push(f("totp", FieldType::Totp, "otpauth://totp/Google:ana.moura.demo%40gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google"));
        add(personal, gmail)?;
        add(personal, login("Nubank", "https://app.nubank.com.br", "123.456.789-09", "m7#Qe2!vZr9@Tx4^kP", &["finanças"], true))?;
        add(personal, login("Netflix", "https://www.netflix.com/br", "ana.moura.demo@gmail.com", "netflix123", &["streaming"], false))?;
        add(personal, login("Spotify", "https://open.spotify.com", "anamoura", "netflix123", &["streaming"], false))?;
        add(personal, login("Mercado Livre", "https://www.mercadolivre.com.br", "ana.moura.demo@gmail.com", "Sabia-Laranja-Tatu4-Rede", &["compras"], false))?;
        add(personal, login("iFood", "https://www.ifood.com.br", "ana.moura.demo@gmail.com", "ifood2019", &["compras"], false))?;
        add(personal, login("gov.br", "https://acesso.gov.br", "123.456.789-09", "Cajueiro.Farol.Moenda.Tapete7", &["governo"], true))?;
        add(personal, login("Amazon", "https://www.amazon.com.br", "ana.moura.demo@gmail.com", "hT3$wQ9!mZ2@pL8r", &["compras"], false))?;
        add(personal, login("Instagram", "https://www.instagram.com", "anamoura.fotos", "Carambola-Vento-Esquina2", &["redes sociais"], false))?;
        add(personal, login("Uber", "https://m.uber.com", "+55 11 98765-4321", "senha123", &[], false))?;
        add(personal, login("Itaú", "https://www.itau.com.br", "Agência 0001 · Conta 12345-6", "4815", &["finanças"], false))?;
        add(personal, login("Steam", "https://store.steampowered.com", "anamoura_games", "Pipoca!Estrela!Guarda-chuva9", &["jogos"], false))?;

        let mut gh = login("GitHub", "https://github.com", "ana-moura-dev", "rV8&kN2#qW5!zX7@", &["trabalho/dev"], true);
        gh.fields.push(f("totp", FieldType::Totp, "otpauth://totp/GitHub:ana-moura-dev?secret=KRSXG5CTMVRXEZLUKN2XAZLSKNSWC4DF&issuer=GitHub"));
        add(work, gh)?;
        add(work, login("Slack", "https://acme-demo.slack.com", "ana@acme.example", "Onda-Coqueiro-Jangada3", &["trabalho"], false))?;
        add(work, login("Jira", "https://acme-demo.atlassian.net", "ana@acme.example", "Onda-Coqueiro-Jangada3", &["trabalho"], false))?;
        add(work, login("Google Workspace (Acme)", "https://admin.google.com", "ana@acme.example", "wQ4!nL9#tR2@", &["trabalho"], false))?;
        add(work, login("Figma", "https://www.figma.com", "ana@acme.example", "figma2021", &["trabalho/design"], false))?;
        add(work, login("AWS Console", "https://console.aws.amazon.com", "ana.admin", "Zb8#Lm3!Qr6$Tw1^", &["trabalho/dev", "infra"], false))?;

        // Cards (test numbers from card networks' public test ranges).
        add(
            personal,
            item(
                ItemKind::Card,
                "Nubank Ultravioleta",
                &[],
                &["finanças"],
                true,
                vec![
                    f("cardholder", FieldType::Text, "ANA B MOURA"),
                    f("number", FieldType::CardNumber, "5555555555554444"),
                    f("expiry", FieldType::MonthYear, "08/31"),
                    f("cvv", FieldType::Pin, "123"),
                    f("cardType", FieldType::Select, "Crédito"),
                    f("issuer", FieldType::Text, "Nubank"),
                    f("phone", FieldType::Phone, "0800 591 2117"),
                ],
                "Cartão de demonstração — número de teste.",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::Card,
                "Itaú Visa",
                &[],
                &["finanças"],
                false,
                vec![
                    f("cardholder", FieldType::Text, "ANA B MOURA"),
                    f("number", FieldType::CardNumber, "4111111111111111"),
                    f("expiry", FieldType::MonthYear, "03/29"),
                    f("cvv", FieldType::Pin, "456"),
                    f("pin", FieldType::Pin, "8264"),
                    f("cardType", FieldType::Select, "Múltiplo"),
                    f("issuer", FieldType::Text, "Itaú"),
                ],
                "",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::Card,
                "Vale-alimentação",
                &[],
                &["família"],
                false,
                vec![
                    f("cardholder", FieldType::Text, "ANA B MOURA"),
                    f("number", FieldType::CardNumber, "6362970000457013"),
                    f("expiry", FieldType::MonthYear, "11/27"),
                    f("cardType", FieldType::Select, "Vale-alimentação"),
                    f("issuer", FieldType::Text, "Alelo"),
                ],
                "",
            ),
        )?;

        // Identity & documents.
        add(
            personal,
            item(
                ItemKind::Identity,
                "Meus dados",
                &[],
                &[],
                true,
                vec![
                    fs("fullName", FieldType::Text, "Ana Beatriz Moura", "personal"),
                    fs("birthDate", FieldType::Date, "1991-04-17", "personal"),
                    fs("cpf", FieldType::Cpf, "12345678909", "personal"),
                    fs("rg", FieldType::Text, "12.345.678-9", "personal"),
                    fs("email", FieldType::Email, "ana.moura.demo@gmail.com", "personal"),
                    fs("phone", FieldType::Phone, "11987654321", "personal"),
                    fs("cep", FieldType::Cep, "01310100", "address"),
                    fs("street", FieldType::Text, "Avenida Paulista", "address"),
                    fs("number", FieldType::Text, "1000", "address"),
                    fs("complement", FieldType::Text, "Apto 82", "address"),
                    fs("district", FieldType::Text, "Bela Vista", "address"),
                    fs("city", FieldType::Text, "São Paulo", "address"),
                    fs("state", FieldType::Select, "SP", "address"),
                ],
                "",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::Document,
                "CNH da Ana",
                &[],
                &["documentos"],
                false,
                vec![
                    f("docType", FieldType::Select, "CNH"),
                    f("number", FieldType::Text, "01234567890"),
                    f("fullName", FieldType::Text, "Ana Beatriz Moura"),
                    f("issuer", FieldType::Text, "DETRAN-SP"),
                    f("issueDate", FieldType::Date, "2022-06-10"),
                    f("expiryDate", FieldType::Date, "2032-06-10"),
                    f("category", FieldType::Text, "AB"),
                ],
                "",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::Document,
                "Passaporte",
                &[],
                &["documentos", "viagem"],
                false,
                vec![
                    f("docType", FieldType::Select, "Passaporte"),
                    f("number", FieldType::Text, "FZ123456"),
                    f("fullName", FieldType::Text, "Ana Beatriz Moura"),
                    f("issuer", FieldType::Text, "Polícia Federal"),
                    f("issueDate", FieldType::Date, "2019-02-20"),
                    f("expiryDate", FieldType::Date, "2029-02-19"),
                ],
                "",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::Document,
                "RG do Theo",
                &[],
                &["documentos", "família/filhos"],
                false,
                vec![
                    f("docType", FieldType::Select, "RG"),
                    f("number", FieldType::Text, "98.765.432-1"),
                    f("fullName", FieldType::Text, "Theo Moura Lima"),
                    f("issuer", FieldType::Text, "SSP/SP"),
                    f("issueDate", FieldType::Date, "2021-09-01"),
                ],
                "",
            ),
        )?;

        // Home & family.
        add(
            family,
            item(
                ItemKind::Wifi,
                "Wi-Fi de casa",
                &[],
                &["casa"],
                true,
                vec![f("ssid", FieldType::Text, "CASA_MOURA_5G"), f("password", FieldType::Concealed, "Jabuticaba-Rede-Varal7"), f("security", FieldType::Select, "WPA3")],
                "Roteador na estante da sala. Senha do painel do roteador: na etiqueta de baixo.",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::Wifi,
                "Wi-Fi da casa de praia",
                &[],
                &["casa"],
                false,
                vec![f("ssid", FieldType::Text, "PRAIA_UBATUBA"), f("password", FieldType::Concealed, "ubatuba2020"), f("security", FieldType::Select, "WPA2")],
                "",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::HealthPlan,
                "Plano de saúde da família",
                &[],
                &["família", "saúde"],
                false,
                vec![
                    f("provider", FieldType::Text, "Operadora Exemplo Saúde"),
                    f("plan", FieldType::Text, "Família Plus Enfermaria"),
                    f("memberNumber", FieldType::Text, "0 123 456789012 3"),
                    f("holder", FieldType::Text, "Ana Beatriz Moura"),
                    f("validUntil", FieldType::Date, "2027-12-31"),
                    f("cns", FieldType::Text, "898 0012 3456 7890"),
                    f("phone", FieldType::Phone, "08007770000"),
                ],
                "",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::Vehicle,
                "Carro da família",
                &[],
                &["família"],
                false,
                vec![
                    f("model", FieldType::Text, "Onix Plus LT"),
                    f("year", FieldType::Number, "2022"),
                    f("plate", FieldType::Text, "BRA2E19"),
                    f("renavam", FieldType::Text, "01234567890"),
                    f("chassis", FieldType::Text, "9BGEA48A0MG000000"),
                    f("color", FieldType::Text, "Prata"),
                    f("insurer", FieldType::Text, "Seguradora Exemplo"),
                    f("policyNumber", FieldType::Text, "AP-2025-000123"),
                ],
                "Revisão a cada 10.000 km.",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::BankAccount,
                "Conta corrente Itaú",
                &[],
                &["finanças"],
                false,
                vec![
                    f("bank", FieldType::Text, "341 · Itaú Unibanco"),
                    f("branch", FieldType::Text, "0001"),
                    f("accountNumber", FieldType::Text, "12345-6"),
                    f("accountType", FieldType::Select, "Corrente"),
                    f("holder", FieldType::Text, "Ana Beatriz Moura"),
                    f("pixKeys", FieldType::Multiline, "ana.moura.demo@gmail.com\n+55 11 98765-4321"),
                    f("appPassword", FieldType::Concealed, "i7@Wq2#Lp9"),
                    f("cardPassword", FieldType::Pin, "4815"),
                ],
                "",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::Note,
                "Combinação do cadeado da academia",
                &[],
                &[],
                false,
                vec![],
                "Cadeado azul: 3-7-1-9\nArmário 42, fila de cima.\n\nSe esquecer, a recepção abre com documento.",
            ),
        )?;
        add(
            family,
            item(
                ItemKind::Note,
                "Senhas e contatos da escola",
                &[],
                &["família/filhos"],
                false,
                vec![],
                "Portal do aluno: matrícula 2024-0931\nSecretaria: (11) 3333-0000\nSenha da agenda digital na etiqueta da mochila.",
            ),
        )?;
        add(
            personal,
            item(
                ItemKind::License,
                "Microsoft 365 Personal",
                &[],
                &["software"],
                false,
                vec![
                    f("product", FieldType::Text, "Microsoft 365 Personal"),
                    f("version", FieldType::Text, "Assinatura anual"),
                    f("licenseKey", FieldType::Concealed, "XXXXX-DEMO1-DEMO2-DEMO3-XXXXX"),
                    f("email", FieldType::Email, "ana.moura.demo@gmail.com"),
                    f("purchaseDate", FieldType::Date, "2025-03-02"),
                ],
                "",
            ),
        )?;

        // Work tech.
        add(
            work,
            item(
                ItemKind::Server,
                "VPS produção",
                &[],
                &["trabalho/dev", "infra"],
                false,
                vec![
                    f("host", FieldType::Text, "203.0.113.24"),
                    f("port", FieldType::Number, "22"),
                    f("protocol", FieldType::Select, "SSH"),
                    f("username", FieldType::Text, "deploy"),
                    f("password", FieldType::Concealed, "Tq5#Wn8!Lr2@Vz6$"),
                ],
                "",
            ),
        )?;
        add(
            work,
            item(
                ItemKind::Database,
                "Postgres produção",
                &[],
                &["trabalho/dev", "infra"],
                false,
                vec![
                    f("dbType", FieldType::Select, "PostgreSQL"),
                    f("host", FieldType::Text, "db.acme.example"),
                    f("port", FieldType::Number, "5432"),
                    f("database", FieldType::Text, "acme_prod"),
                    f("username", FieldType::Text, "acme_app"),
                    f("password", FieldType::Concealed, "pG7!xQ2#mV9@wL4$"),
                    f("options", FieldType::Text, "sslmode=require"),
                ],
                "",
            ),
        )?;
        add(
            work,
            item(
                ItemKind::ApiCredential,
                "Stripe (teste)",
                &[],
                &["trabalho/dev"],
                false,
                vec![
                    f("service", FieldType::Text, "Stripe"),
                    f("keyId", FieldType::Text, "pk_test_demo"),
                    f("apiKey", FieldType::Concealed, "sk_test_DEMO_4eC39HqLyjWDarjtT1zdp7dc"),
                    f("environment", FieldType::Select, "Teste"),
                ],
                "",
            ),
        )?;
        add(
            work,
            item(
                ItemKind::SshKey,
                "Notebook do trabalho",
                &[],
                &["trabalho/dev"],
                false,
                vec![
                    f("privateKey", FieldType::SshPrivateKey, "-----BEGIN OPENSSH PRIVATE KEY-----\nDEMONSTRACAO-NAO-E-UMA-CHAVE-REAL\n-----END OPENSSH PRIVATE KEY-----"),
                    f("publicKey", FieldType::Multiline, "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDemonstracaoNaoEUmaChaveReal ana@acme"),
                    f("fingerprint", FieldType::Text, "SHA256:demo+fingerprint/0123456789"),
                    f("keyType", FieldType::Select, "Ed25519"),
                ],
                "",
            ),
        )?;
        let mut c = item(
            ItemKind::Custom,
            "Portaria do prédio",
            &[],
            &["casa"],
            false,
            vec![custom("Código do portão", FieldType::Pin, "2580"), custom("Telefone do zelador", FieldType::Phone, "11912345678")],
            "",
        );
        c.fields.push(custom("Horário da mudança", FieldType::Text, "Seg a sáb, 8h às 17h"));
        add(personal, c)?;

        Ok(n)
    }
}

use std::sync::Arc;
use tauri::State;

#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub async fn dev_seed(state: State<'_, Arc<AppState>>) -> AppResult<usize> {
    #[cfg(debug_assertions)]
    {
        state.with_account(|a| Ok(data::seed(a)?))
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = state;
        Err(AppError::new("unavailable", "Indisponível nesta versão."))
    }
}
