mod auth;

use base_config::AppConfig;
use base_server::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .json()
        .init();

    let config = AppConfig::from_env()?;
    tracing::info!(project = %config.project, port = config.server_port, "starting attome server");

    let db = base_db::create_pool(&config).await?;
    tracing::info!("postgres pool connected");

    sqlx::migrate!().run(&db).await?;
    tracing::info!("migrations applied");

    let redis = base_cache::create_redis_pool(&config).await?;

    let state = AppState::new(config.clone(), db, redis);

    // build_app returns Router<()> (state already set); auth routes need .with_state() first.
    let app = xrm_server::build_app(state.clone())
        .merge(auth::routes().with_state(state.clone()));

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
