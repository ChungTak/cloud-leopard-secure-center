//! ETag and conditional request helpers.

use axum::{
    extract::FromRequestParts,
    http::{HeaderValue, header, request::Parts},
};
use foundation::Revision;

use crate::error::AppError;

/// Strong ETag based on an aggregate revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ETag(pub Revision);

impl ETag {
    /// Build a response `ETag` header value.
    pub fn header_value(&self) -> Result<HeaderValue, AppError> {
        let value = format!("\"{}\"", self.0.0);
        HeaderValue::from_str(&value).map_err(|_| AppError::Internal)
    }
}

/// `If-Match` precondition extracted from the request.
#[derive(Debug, Clone)]
pub struct IfMatch {
    revisions: Vec<Revision>,
    wildcard: bool,
}

impl IfMatch {
    /// Verify the precondition against the current revision.
    pub fn verify(&self, current: Revision) -> Result<(), AppError> {
        if self.wildcard {
            return Ok(());
        }
        if self.revisions.contains(&current) {
            Ok(())
        } else {
            Err(AppError::VersionMismatch)
        }
    }
}

impl<S> FromRequestParts<S> for IfMatch
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::IF_MATCH)
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::VersionMismatch)?;

        if header.trim() == "*" {
            return Ok(Self {
                revisions: Vec::new(),
                wildcard: true,
            });
        }

        let revisions = parse_etags(header)?;
        Ok(Self {
            revisions,
            wildcard: false,
        })
    }
}

fn parse_etags(header: &str) -> Result<Vec<Revision>, AppError> {
    let mut revisions = Vec::new();
    for token in header.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let inner = token
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(|| AppError::BadRequest {
                field: "If-Match".to_string(),
                message: "invalid ETag format".to_string(),
            })?;
        let rev = inner.parse::<u64>().map_err(|_| AppError::BadRequest {
            field: "If-Match".to_string(),
            message: "invalid revision in ETag".to_string(),
        })?;
        revisions.push(Revision(rev));
    }
    if revisions.is_empty() {
        return Err(AppError::BadRequest {
            field: "If-Match".to_string(),
            message: "no valid ETags provided".to_string(),
        });
    }
    Ok(revisions)
}
