use std::time::Duration;

use serde::Deserialize;
use serde::de::Error as _;

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub admin_address: String,
    pub address: String,
    pub jwt: Jwt,
    #[serde(default, deserialize_with = "deserialize_optional_duration")]
    pub latency_inject: Option<Duration>,
}

fn deserialize_optional_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DurationRepr {
        Text(String),
        Millis(u64),
    }

    let value = Option::<DurationRepr>::deserialize(deserializer)?;
    match value {
        Some(DurationRepr::Text(text)) => humantime::parse_duration(&text).map(Some).map_err(D::Error::custom),
        Some(DurationRepr::Millis(ms)) => Ok(Some(Duration::from_millis(ms))),
        None => Ok(None),
    }
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
