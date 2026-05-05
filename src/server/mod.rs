pub mod api;
pub mod db;
pub mod models;
pub mod queue;
pub mod state;

use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::cpu::CpuConfig;
use state::AppState;

/// Start the web server with Axum.
pub async fn run(bind: &str, db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pool = db::open(db_path)?;
    let (state, _shutdown_rx) = AppState::new(pool.clone());

    // Start queue manager in background
    let qm = queue::QueueManager::new(pool);
    tokio::spawn(async move {
        qm.run().await;
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Serve static files first, then fall back to index.html for SPA routing
    let spa = ServeFile::new("static/index.html");

    let app = api::router()
        .layer(cors)
        .nest_service("/assets", ServeDir::new("static/assets"))
        .fallback_service(spa)
        .with_state(state);

    let cpu = CpuConfig::detect();
    eprintln!(
        "mc-keygen server: http://{}  db={}  cpu={}",
        bind,
        db_path,
        cpu.summary()
    );

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
