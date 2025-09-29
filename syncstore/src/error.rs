use std::any::Any;

use salvo::{Scribe, http::StatusCode, oapi::EndpointOutRegister};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("backend error: {0}")]
    Backend(String),

    #[error("not found")]
    NotFound,

    #[error("validation error: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type StoreResult<T> = std::result::Result<T, StoreError>;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("store error: {0}")]
    StoreError(#[from] StoreError),

    #[error("jwt error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("internal server error: {0}")]
    InternalServerError(String),
}

pub type ServiceResult<T> = std::result::Result<T, ServiceError>;

impl Scribe for ServiceError {
    fn render(self, res: &mut salvo::Response) {
        match self {
            ServiceError::Unauthorized(msg) => {
                res.status_code(StatusCode::UNAUTHORIZED);
                res.render(msg);
            }
            ServiceError::StoreError(store_error) => todo!(),
            ServiceError::JwtError(error) => todo!(),
            ServiceError::InternalServerError(msg) => {
                res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
                res.render(msg);
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
