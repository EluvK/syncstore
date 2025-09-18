use salvo::{Scribe, http::StatusCode};
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
        }
    }
}
