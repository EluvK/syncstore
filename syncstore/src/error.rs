use std::any::Any;

use r2d2_sqlite::rusqlite;
use salvo::{Scribe, http::StatusCode, oapi::EndpointOutRegister};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("backend error: {0}")]
    Backend(String),

    #[error("{0} not found")]
    NotFound(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type StoreResult<T> = std::result::Result<T, StoreError>;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    // JWT generation or validation error
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

pub type ServiceResult<T> = std::result::Result<T, ServiceError>;

impl Scribe for ServiceError {
    fn render(self, res: &mut salvo::Response) {
        res.render(format!("{self}"));
        match self {
            ServiceError::Unauthorized(_) => {
                res.status_code(StatusCode::UNAUTHORIZED);
            }
            ServiceError::StoreError(store_error) => match &store_error {
                StoreError::NotFound(_) => {
                    res.status_code(StatusCode::NOT_FOUND);
                }
                StoreError::Validation(_) => {
                    res.status_code(StatusCode::BAD_REQUEST);
                }
                _ => {
                    res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                }
            },
            ServiceError::JwtError(_) => {
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            }
            ServiceError::InternalServerError(_) => {
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
}

impl EndpointOutRegister for ServiceError {
    fn register(components: &mut salvo::oapi::Components, operation: &mut salvo::oapi::Operation) {}
}

// for depot.get/obtain
impl From<Option<&Box<dyn Any + Send + Sync>>> for ServiceError {
    fn from(value: Option<&Box<dyn Any + Send + Sync>>) -> Self {
        ServiceError::InternalServerError(
            value
                .and_then(|v| v.downcast_ref::<String>())
                .map(|s| s.clone())
                .unwrap_or_else(|| "Unknown error".to_string()),
        )
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Backend(error.to_string())
    }
}

impl From<r2d2::Error> for StoreError {
    fn from(error: r2d2::Error) -> Self {
        StoreError::Backend(error.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        StoreError::Backend(error.to_string())
    }
}

impl From<chrono::ParseError> for StoreError {
    fn from(error: chrono::ParseError) -> Self {
        StoreError::Backend(error.to_string())
    }
}
