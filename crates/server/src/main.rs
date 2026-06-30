mod auth;

use base_config::AppConfig;
use base_server::AppState;
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

    xrm_entity::reload_registry(&state.base.db, &state.entities).await?;
    xrm_server::seed_core_entities(&state.base.db, &state.entities).await?;

    // ── Build the XRM platform ─────────────────────────────────────────────────
    // Set XRM_DOMAIN env var (or call .domain("...")) to bind the license to your domain.
    // Set XRM_LICENSE_FILE env var to point to your license.json (default: ./license.json).
    // Replace the noop service backends with real implementations for production.
    let system = XrmSystem::builder(state)
        // .domain("my.client.com")
        // .notifications(Arc::new(SendGridService::new(&config)))
        // .ai(Arc::new(OpenAiService::new(&config)))
        .build()
        .await?;

    let base_state = system.state().base.clone();

    let app = system.core_routes()
        .merge(auth::routes().with_state(base_state))
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
