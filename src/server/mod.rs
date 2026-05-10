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

    // Clean up any stuck running/paused jobs from previous run
    {
        let db = pool.lock().unwrap_or_else(|e| e.into_inner());
        let jobs = db::list_jobs(&db).unwrap_or_default();
        for job in &jobs {
            if job.status == crate::server::models::JobStatus::Running
                || job.status == crate::server::models::JobStatus::Paused
            {
                let _ = db::update_job(
                    &db,
                    &crate::server::models::Job {
                        status: crate::server::models::JobStatus::Stopped,
                        notes: Some("Server restarted — job interrupted".to_string()),
                        ..job.clone()
                    },
                );
            }
        }
    }

    let (state, _shutdown_rx) = AppState::new(pool.clone());

    // Start queue manager in background
    let qm = queue::QueueManager::new(
        pool,
        state.active_job_cancel.clone(),
        state.active_job_id.clone(),
    );
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
