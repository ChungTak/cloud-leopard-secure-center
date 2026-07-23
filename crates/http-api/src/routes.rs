//! HTTP route handlers.

use axum::{Json, Router, routing::get};

use crate::dto::HealthDto;

/// Health check handler.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthDto)
    )
)]
pub(crate) async fn health() -> Json<HealthDto> {
    Json(HealthDto {
        status: "healthy".to_string(),
        component: "http-api".to_string(),
    })
}

/// Public router exposing health and placeholder resource routes.
pub fn router() -> Router {
    Router::new().route("/health", get(health))
}
