//! OpenAPI 3.1 document and snapshot helpers.

use utoipa::OpenApi;

use crate::{dto::HealthDto, routes};

/// Cloud Leopard Secure Center HTTP API.
#[derive(OpenApi)]
#[openapi(
    info(title = "Cloud Leopard Secure Center", version = "0.1.0"),
    paths(routes::health),
    components(schemas(HealthDto))
)]
pub struct ApiDoc;

impl ApiDoc {
    /// Return the OpenAPI document as a JSON string.
    pub fn json() -> String {
        ApiDoc::openapi().to_pretty_json().unwrap_or_default()
    }
}
