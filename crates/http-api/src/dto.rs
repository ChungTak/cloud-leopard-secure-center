//! HTTP request/response DTOs and domain mappers.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Generic API problem details sent by this service.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailsDto {
    #[schema(rename = "type")]
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

/// Health status returned by the root endpoint.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    pub status: String,
    pub component: String,
}
