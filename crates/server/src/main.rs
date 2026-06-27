mod auth;

use base_config::AppConfig;
use base_server::AppState;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use xrm_foundation::XrmState;

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

    sqlx::migrate!().run(&db).await?;
    tracing::info!("migrations applied");

    let redis   = base_cache::create_redis_pool(&config).await?;
    let storage = base_storage::create_storage_client(&config).await?;
    tracing::info!("storage client ready");

    let base  = AppState::new(config.clone(), db, redis, storage);
    let state = XrmState::new(base);

    xrm_entity::reload_registry(&state.base.db, &state.entities).await?;

    let app = xrm_server::build_app(state.clone())
        .merge(auth::routes().with_state(state.base.clone()))
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
