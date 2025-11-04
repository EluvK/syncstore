use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StoreError;

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

/// DataItemDocument
/// diff with DataItem: the body is String
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataItemDocument {
    pub id: Id,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: Uid,
    pub unique: Option<String>,
    pub parent_id: Option<String>,
    pub body: String,
}

impl TryFrom<DataItemDocument> for DataItem {
    type Error = StoreError;

    fn try_from(value: DataItemDocument) -> std::result::Result<Self, Self::Error> {
        let body: serde_json::Value = serde_json::from_str(&value.body)?;
        Ok(Self {
            id: value.id,
            created_at: value.created_at,
            updated_at: value.updated_at,
            owner: value.owner,
            unique: value.unique,
            parent_id: value.parent_id,
            body,
        })
    }
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
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Read,
    Edit,
    Write,
    FullAccess,
}

impl AccessLevel {
    pub fn contains(&self, other: &AccessLevel) -> bool {
        match (self, other) {
            (AccessLevel::FullAccess, _)
            | (AccessLevel::Write, AccessLevel::Write | AccessLevel::Read | AccessLevel::Edit)
            | (AccessLevel::Edit, AccessLevel::Edit | AccessLevel::Read)
            | (AccessLevel::Read, AccessLevel::Read) => true,
            _ => false,
        }
    }
}
