use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::domain::DecisionSide;

use super::super::{WebState, apply_decision};

pub(crate) async fn left(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Left).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn right(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Right).await;
    StatusCode::NO_CONTENT
}
