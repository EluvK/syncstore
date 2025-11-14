use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub admin_address: String,
    pub address: String,
    pub jwt: Jwt,
}

#[derive(Debug, Deserialize)]
pub struct Jwt {
    pub access_secret: String,
    pub refresh_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct StoreConfig {
    pub directory: String,
}
