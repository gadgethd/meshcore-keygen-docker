pub mod benchmarks;
pub mod estimate;
pub mod jobs;
pub mod results;
pub mod settings;

use super::state::AppState;
use axum::Router;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(jobs::router())
        .merge(results::router())
        .merge(estimate::router())
        .merge(benchmarks::router())
        .merge(settings::router())
}
