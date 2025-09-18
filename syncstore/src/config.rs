use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub address: String,
    pub jwt: Jwt,
}

#[derive(Debug, Deserialize)]
pub struct Jwt {
    pub access_secret: String,
    pub refresh_secret: String,
}
