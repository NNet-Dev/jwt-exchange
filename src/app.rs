//! Application bootstrap and HTTP server configuration.
//!
//! Orchestrates database initialization, JWKS loading, RSA key resolution,
//! background tasks (JTI purge, audit purge, Splunk exporter), and the
//! actix-web server with request ID middleware.

use actix_web::{web, App};
use jsonwebtoken::EncodingKey;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::auth::jwks::JwksCache;
use crate::auth::signing;
use crate::config::AppConfig;
use crate::logging::splunk::SplunkEvent;

/// Shared application state passed to all handlers.
pub struct AppState {
    pub config: AppConfig,
    pub jwks_cache: JwksCache,
    pub encoding_key: EncodingKey,
    pub public_cert: String,
    pub http_client: Client,
    pub pool: SqlitePool,
    pub splunk_tx: Option<mpsc::Sender<SplunkEvent>>,
}

/// A fully bootstrapped application ready to serve requests.
pub struct BootedApp {
    pub host: String,
    pub port: u16,
    pub server: actix_web::dev::Server,
}

/// Bootstrap the entire application: initialise DB, JWKS, signing keys,
/// background tasks, Splunk exporter, and the HTTP server.
pub async fn bootstrap(config: AppConfig) -> anyhow::Result<BootedApp> {
    // Create HTTP client
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("failed to create HTTP client: {e}"))?;

    // Initialize database
    let pool = crate::db::pool::create_pool(&config.db_path).await?;
    info!("Database initialized at {}", config.db_path);

    // Load JWKS
    let jwks_cache = crate::auth::jwks::fetch_and_cache_jwks(&config, &http_client).await?;

    // Start JWKS refresh background task
    crate::auth::jwks::start_jwks_refresh_task(jwks_cache.clone(), http_client.clone()).await;

    // Resolve RSA signing key pair (load existing or auto-generate)
    let rsa_source = config.rsa_source();
    let key_pair = signing::resolve_key_pair(&rsa_source)?;

    // Start background purge tasks (db-scoped workers)
    crate::db::audit::start_purge_task(pool.clone(), config.log_retention_days);
    crate::db::audit::start_jti_purge_task(pool.clone());

    // Start Splunk HEC exporter if configured
    let splunk_tx: Option<mpsc::Sender<SplunkEvent>> =
        match (&config.splunk_hec_url, &config.splunk_hec_token) {
            (Some(url), Some(token)) => {
                let (tx, rx) = mpsc::channel::<SplunkEvent>(1000);
                crate::logging::splunk::start_splunk_exporter(
                    url.clone(),
                    token.clone(),
                    config.splunk_hec_skip_tls_verify,
                    rx,
                );
                info!(
                    url,
                    tls_skip = config.splunk_hec_skip_tls_verify,
                    "Splunk HEC exporter started"
                );
                Some(tx)
            }
            _ => {
                info!("Splunk HEC not configured, export disabled");
                None
            }
        };

    // Build shared state
    let app_state = web::Data::new(AppState {
        config: config.clone(),
        jwks_cache,
        encoding_key: key_pair.encoding_key,
        public_cert: key_pair.public_cert_pem,
        http_client,
        pool: pool.clone(),
        splunk_tx,
    });

    // F17: Clone pool for shutdown handler before it's consumed by pool_data
    let pool_for_shutdown = pool.clone();
    let pool_data = web::Data::new(pool);

    let host = config.listen_host.clone();
    let port = config.listen_port;

    info!("Starting server on {host}:{port}");

    let server = actix_web::HttpServer::new(move || {
        App::new()
            // F18: Request ID middleware — injects X-Request-Id into all responses
            .wrap(crate::middleware::RequestIdMiddleware)
            .app_data(app_state.clone())
            .app_data(pool_data.clone())
            .app_data(web::Data::new(app_state.jwks_cache.clone()))
            .route(
                "/api/v1/exchange",
                web::post().to(crate::handlers::exchange::exchange),
            )
            .route("/api/v1/cert", web::get().to(crate::handlers::cert::cert))
            .route(
                "/api/v1/health",
                web::get().to(crate::handlers::health::health),
            )
    })
    .bind((host.as_str(), port))?
    .run();

    // F17: Register graceful shutdown handler to checkpoint WAL before exit.
    let shutdown_handle = server.handle();
    tokio::spawn(async move {
        // Wait for shutdown signal (SIGINT/SIGTERM)
        tokio::signal::ctrl_c().await.ok();
        warn!("Shutdown signal received, flushing database...");
        crate::db::pool::checkpoint_and_close(&pool_for_shutdown).await;
        shutdown_handle.stop(true).await;
    });

    Ok(BootedApp { host, port, server })
}
