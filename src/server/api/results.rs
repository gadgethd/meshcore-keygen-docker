use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};

use crate::server::models::ResultRecord;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/results", axum::routing::get(list))
        .route("/api/results/{id}", axum::routing::delete(delete_result))
}

async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<ResultRecord>>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let settings = crate::server::db::load_settings(&db);
    let mut results =
        crate::server::db::list_results(&db).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Redact secrets if hide_secrets is enabled
    if settings.hide_secrets {
        for r in &mut results {
            r.private_key = "[hidden]".to_string();
            r.candidate_seed = None;
            r.master_seed = None;
        }
    }
    Ok(Json(results))
}

async fn delete_result(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> StatusCode {
    let db = state.db.lock().unwrap();
    crate::server::db::delete_result(&db, &id).ok();
    StatusCode::NO_CONTENT
}
