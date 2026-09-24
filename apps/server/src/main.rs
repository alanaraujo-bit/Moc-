//! Mocó sync server.
//!
//! Stores only what devices already encrypted, verifies Ed25519 login signatures, keeps
//! per-account sequence numbers for incremental sync, and never holds a key that could
//! decrypt anything. See DECISIONS.md (D-014).

mod account;
mod auth;
mod error;
mod files;
mod ratelimit;
mod secrets;
mod sharing;
mod sync;
#[cfg(test)]
mod tests;

use axum::routing::{delete, get, post, put};
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

pub struct AppState {
    pub db: PgPool,
    pub secrets: secrets::ServerSecrets,
    pub limits: ratelimit::Limiter,
}

pub type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/v1/prelogin", post(account::prelogin))
        .route("/v1/register", post(account::register))
        .route("/v1/login/challenge", post(account::challenge))
        .route("/v1/login", post(account::login))
        .route("/v1/logout", post(account::logout))
        .route("/v1/me", get(account::me))
        .route("/v1/record", put(account::put_record))
        .route("/v1/account", delete(account::delete_account))
        .route("/v1/devices/{id}", delete(account::revoke_device))
        .route("/v1/2fa/setup", post(account::totp_setup))
        .route("/v1/2fa/enable", post(account::totp_enable))
        .route("/v1/2fa/disable", post(account::totp_disable))
        .route("/v1/sync/pull", get(sync::pull))
        .route("/v1/sync/push", post(sync::push))
        .route("/v1/attachments/{id}", put(files::put).get(files::get).delete(files::delete))
        .route("/v1/people", get(sharing::people))
        .route("/v1/vaults/{vault}/members", get(sharing::list_members))
        .route("/v1/vaults/{vault}/members/{member}", put(sharing::put_member).delete(sharing::delete_member))
        .route("/v1/shared", get(sharing::shared))
        .route("/v1/shared/{owner}/{vault}/pull", get(sharing::shared_pull))
        .route("/v1/shared/{owner}/{vault}/push", post(sharing::shared_push))
        .route(
            "/v1/shared/{owner}/{vault}/attachments/{id}",
            put(sharing::shared_attachment_put).get(sharing::shared_attachment_get).delete(sharing::shared_attachment_delete),
        )
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(RequestBodyLimitLayer::new(64 * 1024 * 1024))
        .layer(CorsLayer::new()) // desktop and mobile clients don't need CORS; deny browsers
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
        .expect("database");
    sqlx::migrate!("./migrations").run(&db).await.expect("migrations");

    let state = Arc::new(AppState {
        db,
        secrets: secrets::ServerSecrets::from_env(),
        limits: ratelimit::Limiter::default(),
    });

    // Housekeeping: expired challenges and sessions.
    let db = state.db.clone();
    tokio::spawn(async move {
        loop {
            let _ = sqlx::query("DELETE FROM challenges WHERE expires_at < now()").execute(&db).await;
            let _ = sqlx::query("DELETE FROM sessions WHERE expires_at < now()").execute(&db).await;
            tokio::time::sleep(Duration::from_secs(600)).await;
        }
    });

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.expect("bind");
    tracing::info!("mocó server listening on {port}");
    axum::serve(listener, router(state).into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}
