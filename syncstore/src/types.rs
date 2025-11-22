use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StoreError;

/// Identifier type used across the store.
pub type Id = String;
pub type Uid = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSchema {
    pub username: String,
    pub password: String,
    pub avatar_url: Option<String>,
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

// you might want to update the `AclManager::new(), schema enums as well when modifying this.`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    /// Can only read existing data.
    Read,
    /// Can only update existing data, cannot create new data at all.
    Update,
    /// Can read and create new data as sibling, but cannot update existing data.
    Create,
    /// Can read, create new data and update existing data. Cannot delete.
    Write,
    /// Can do all operations, including delete.
    FullAccess,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ACLMask: u8 {
        const READ_ONLY   = 0b00001;
        const UPDATE_ONLY = 0b00010;
        const CREATE_ONLY = 0b00100;
        const DELETE      = 0b01000;
        const FULL_ACCESS = 0b01111;
    }
}

impl From<AccessLevel> for ACLMask {
    fn from(level: AccessLevel) -> Self {
        match level {
            AccessLevel::Read => ACLMask::READ_ONLY,
            AccessLevel::Update => ACLMask::READ_ONLY | ACLMask::UPDATE_ONLY,
            AccessLevel::Create => ACLMask::READ_ONLY | ACLMask::CREATE_ONLY,
            AccessLevel::Write => ACLMask::READ_ONLY | ACLMask::UPDATE_ONLY | ACLMask::CREATE_ONLY,
            AccessLevel::FullAccess => ACLMask::FULL_ACCESS,
        }
    }
}
