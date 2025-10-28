use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifier type used across the store.
pub type Id = String;
pub type Uid = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uid,
    pub name: String,
    pub password: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub avatar_url: Option<String>,
}

impl User {
    pub fn new(name: String, password: String, avatar_url: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            password,
            created_at: now,
            updated_at: now,
            avatar_url,
        }
    }
}

/// Meta fields automatically added to each record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    pub id: Id,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: Uid,
    pub unique: Option<String>,
    pub parent_id: Option<String>,
}

/// DataItem = Meta + Json Value Body, all flatten.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
pub struct DataItem {
    pub id: Id,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: Uid,
    pub unique: Option<String>,
    pub parent_id: Option<String>,
    pub body: serde_json::Value,
}

impl salvo::Scribe for DataItem {
    fn render(self, res: &mut salvo::Response) {
        res.render(salvo::writing::Json(self));
    }
}

impl From<DataItem> for Meta {
    fn from(value: DataItem) -> Self {
        Self {
            id: value.id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            owner: value.owner,
            unique: value.unique,
            parent_id: value.parent_id,
        }
    }
}

impl Meta {
    pub fn new(owner: Uid, unique: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            owner,
            unique,
            parent_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessControl {
    pub data_id: String,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
pub struct Permission {
    pub user: String,
    pub access_level: AccessLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
pub enum AccessLevel {
    Read,
    Edit,
    FullAccess,
}

impl AccessLevel {
    pub fn contains(&self, other: &AccessLevel) -> bool {
        match (self, other) {
            (AccessLevel::FullAccess, _)
            | (AccessLevel::Edit, AccessLevel::Edit | AccessLevel::Read)
            | (AccessLevel::Read, AccessLevel::Read) => true,
            _ => false,
        }
    }
}
