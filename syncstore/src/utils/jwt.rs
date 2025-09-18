use jsonwebtoken::{EncodingKey, Header, decode, encode};
use serde::{Deserialize, Serialize};

use std::sync::OnceLock;

use crate::{config::Jwt, error::ServiceResult};
static ACCESS_TOKEN_SECRET: OnceLock<String> = OnceLock::new();
static REFRESH_TOKEN_SECRET: OnceLock<String> = OnceLock::new();

const ACCESS_TOKEN_EXPIRATION: i64 = 3600; // 1 hour
const REFRESH_TOKEN_EXPIRATION: i64 = 604800; // 7 days

pub fn set_jwt_config(jwt: &Jwt) {
    ACCESS_TOKEN_SECRET.set(jwt.access_secret.clone()).ok();
    REFRESH_TOKEN_SECRET.set(jwt.refresh_secret.clone()).ok();
}

pub fn get_access_secret() -> &'static str {
    ACCESS_TOKEN_SECRET
        .get()
        .map(|s| s.as_str())
        .expect("JWT secret not set")
}

pub fn get_refresh_secret() -> &'static str {
    REFRESH_TOKEN_SECRET
        .get()
        .map(|s| s.as_str())
        .expect("JWT secret not set")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    // (subject): Subject of the JWT (the user)
    pub sub: String,
    // (issued at time): Time at which the JWT was issued;
    // can be used to determine age of the JWT.
    pub iat: i64,
    // (expiration time): Time after which the JWT expires
    pub exp: i64,
    // (type): Type of the JWT, can be used to differentiate between access and refresh tokens
    pub r#type: JwtType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JwtType {
    Access,
    Refresh,
}

impl JwtClaims {
    pub fn access(sub: String, iat: i64, exp: i64) -> Self {
        JwtClaims {
            sub,
            iat,
            exp,
            r#type: JwtType::Access,
        }
    }
    pub fn refresh(sub: String, iat: i64, exp: i64) -> Self {
        JwtClaims {
            sub,
            iat,
            exp,
            r#type: JwtType::Refresh,
        }
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.exp
    }
}

pub fn generate_jwt_token(sub: String) -> ServiceResult<String> {
    let current_time = chrono::Utc::now().timestamp();
    let expiration_time = current_time + ACCESS_TOKEN_EXPIRATION;
    let claims = JwtClaims::access(sub, current_time, expiration_time);
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_access_secret().as_bytes()),
    )?)
}

pub fn generate_refresh_token(sub: String) -> ServiceResult<String> {
    let current_time = chrono::Utc::now().timestamp();
    let expiration_time = current_time + REFRESH_TOKEN_EXPIRATION;
    let claims = JwtClaims::refresh(sub, current_time, expiration_time);
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_refresh_secret().as_bytes()),
    )?)
}

pub fn verify_refresh_token(token: &str) -> ServiceResult<JwtClaims> {
    let token_data = decode::<JwtClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(get_refresh_secret().as_bytes()),
        &jsonwebtoken::Validation::default(),
    )?;
    Ok(token_data.claims)
}
