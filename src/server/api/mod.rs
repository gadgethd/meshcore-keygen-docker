pub mod benchmarks;
pub mod estimate;
pub mod jobs;
pub mod logs;
pub mod results;
pub mod settings;
pub mod ws;

use super::state::AppState;
use axum::Router;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(ws::router())
        .merge(jobs::router())
        .merge(results::router())
        .merge(estimate::router())
        .merge(benchmarks::router())
        .merge(logs::router())
        .merge(settings::router())
}
