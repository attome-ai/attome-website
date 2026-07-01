mod auth;

use axum::middleware;
use base_config::AppConfig;
use base_server::{middleware::auth as authn_mw, AppState};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use xrm_foundation::XrmState;
use xrm_server::XrmSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .json()
        .init();

    let config = AppConfig::from_file()?;
    tracing::info!(port = config.app.port, "starting attome server");

    let db = base_db::create_pool(&config).await?;
    tracing::info!("postgres pool connected");

    let redis   = base_cache::create_redis_pool(&config).await?;
    let storage = base_storage::create_storage_client(&config).await?;
    tracing::info!("infrastructure clients ready");

    let base  = AppState::new(config.clone(), db, redis, storage);
    let state = XrmState::new(base);

    let system = XrmSystem::builder(state)
        .build()
        .await?;

    let base_state = system.state().base.clone();

    // ── Auth routes: public (login/register) + protected (api-keys) ──────────
    let auth_routes = auth::public_routes()
        .merge(
            auth::protected_routes()
                .layer(middleware::from_fn_with_state(base_state.clone(), authn_mw::authn))
        )
        .with_state(base_state);

    let app = system.core_routes()
        .merge(auth_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_credentials(true)
                .allow_methods(AllowMethods::mirror_request())
                .allow_headers(AllowHeaders::mirror_request()),
        );

    let addr     = config.bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}
