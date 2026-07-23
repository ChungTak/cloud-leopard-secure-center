//! Pagination support with opaque, verifiable cursors and a maximum page size.

use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

/// Pagination configuration supplied by the application.
#[derive(Debug, Clone)]
pub struct PaginationConfig {
    /// Maximum number of items per page.
    pub max_page_size: u32,
    /// Secret used to sign cursors.
    pub cursor_secret: Vec<u8>,
}

impl PaginationConfig {
    /// Create a pagination config. `max_page_size` is clamped to a positive
    /// value no larger than `10_000`.
    pub fn new(max_page_size: u32, cursor_secret: impl Into<Vec<u8>>) -> Self {
        const MAX_PAGE_SIZE: u32 = 10_000;
        Self {
            max_page_size: max_page_size.clamp(1, MAX_PAGE_SIZE),
            cursor_secret: cursor_secret.into(),
        }
    }
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            max_page_size: 100,
            cursor_secret: Vec::new(),
        }
    }
}

/// Sort order attached to a cursor.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Sort by the stable identifier ascending.
    #[default]
    IdAsc,
}

/// Cursor payload embedded in the opaque token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CursorPayload {
    offset: u64,
    limit: u32,
    #[serde(default)]
    sort: SortOrder,
}

/// Opaque, verifiable pagination cursor.
#[derive(Debug, Clone)]
pub struct Cursor {
    payload: CursorPayload,
}

/// Wire representation of a cursor token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireCursor {
    p: String,
    m: String,
}

impl Cursor {
    /// Create a cursor at the given offset and limit.
    pub fn new(offset: u64, limit: u32, sort: SortOrder) -> Self {
        Self {
            payload: CursorPayload {
                offset,
                limit,
                sort,
            },
        }
    }

    /// Offset into the result set.
    pub fn offset(&self) -> u64 {
        self.payload.offset
    }

    /// Items requested on this page.
    pub fn limit(&self) -> u32 {
        self.payload.limit
    }

    /// Sort order for the page.
    pub fn sort(&self) -> SortOrder {
        self.payload.sort
    }

    /// Encode the cursor with the given secret.
    pub fn encode(&self, secret: &[u8]) -> Result<String, AppError> {
        if secret.is_empty() {
            return Err(AppError::Internal);
        }
        let payload_json = serde_json::to_string(&self.payload).map_err(|_| AppError::Internal)?;
        let p = Base64UrlUnpadded::encode_string(payload_json.as_bytes());
        let mac = hmac(&p, secret)?;
        let wire = WireCursor {
            p,
            m: Base64UrlUnpadded::encode_string(&mac),
        };
        let json = serde_json::to_string(&wire).map_err(|_| AppError::Internal)?;
        Ok(Base64UrlUnpadded::encode_string(json.as_bytes()))
    }

    /// Parse and verify a cursor token with the given secret.
    pub fn parse(token: &str, secret: &[u8]) -> Result<Self, AppError> {
        if secret.is_empty() {
            return Err(AppError::Internal);
        }
        let json = Base64UrlUnpadded::decode_vec(token).map_err(|_| AppError::BadRequest {
            field: "cursor".to_string(),
            message: "invalid cursor".to_string(),
        })?;
        let wire: WireCursor = serde_json::from_slice(&json).map_err(|_| AppError::BadRequest {
            field: "cursor".to_string(),
            message: "invalid cursor".to_string(),
        })?;
        let actual = Base64UrlUnpadded::decode_vec(&wire.m).map_err(|_| AppError::BadRequest {
            field: "cursor".to_string(),
            message: "invalid cursor".to_string(),
        })?;
        verify_hmac(&wire.p, secret, &actual)?;

        let payload_bytes =
            Base64UrlUnpadded::decode_vec(&wire.p).map_err(|_| AppError::BadRequest {
                field: "cursor".to_string(),
                message: "invalid cursor".to_string(),
            })?;
        let payload: CursorPayload =
            serde_json::from_slice(&payload_bytes).map_err(|_| AppError::BadRequest {
                field: "cursor".to_string(),
                message: "invalid cursor".to_string(),
            })?;
        Ok(Self { payload })
    }
}

fn hmac(message: &str, secret: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| AppError::Internal)?;
    mac.update(message.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_hmac(message: &str, secret: &[u8], signature: &[u8]) -> Result<(), AppError> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| AppError::Internal)?;
    mac.update(message.as_bytes());
    mac.verify_slice(signature)
        .map_err(|_| AppError::BadRequest {
            field: "cursor".to_string(),
            message: "cursor has been tampered".to_string(),
        })
}

/// Query parameters for pagination.
#[derive(Debug, Clone, Deserialize)]
struct PaginationQuery {
    #[serde(default)]
    limit: u32,
    cursor: Option<String>,
}

/// Parsed pagination request.
#[derive(Debug, Clone)]
pub struct Pagination {
    /// Page offset.
    pub offset: u64,
    /// Page limit (clamped to `max_page_size`).
    pub limit: u32,
    /// Stable sort order.
    pub sort: SortOrder,
}

impl Pagination {
    /// Build the next cursor when there are more pages.
    pub fn next_cursor(&self, has_more: bool, secret: &[u8]) -> Result<Option<String>, AppError> {
        if !has_more {
            return Ok(None);
        }
        let next_offset = self
            .offset
            .checked_add(u64::from(self.limit))
            .ok_or(AppError::Internal)?;
        let cursor = Cursor::new(next_offset, self.limit, self.sort);
        cursor.encode(secret).map(Some)
    }
}

impl<S> FromRequestParts<S> for Pagination
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let config = parts
            .extensions
            .get::<Arc<PaginationConfig>>()
            .cloned()
            .unwrap_or_default();

        let Query(query): Query<PaginationQuery> = Query::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::BadRequest {
                field: "pagination".to_string(),
                message: "invalid pagination parameters".to_string(),
            })?;

        let (offset, base_limit, sort) = if let Some(token) = query.cursor {
            let cursor = Cursor::parse(&token, &config.cursor_secret)?;
            (cursor.offset(), cursor.limit(), cursor.sort())
        } else {
            (0, config.max_page_size, SortOrder::default())
        };

        let requested_limit = if query.limit == 0 {
            base_limit
        } else {
            query.limit
        };
        let limit = requested_limit.min(config.max_page_size);

        Ok(Pagination {
            offset,
            limit,
            sort,
        })
    }
}
