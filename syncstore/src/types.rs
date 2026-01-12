use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

/// Identifier type used across the store.
pub type Id = String;
pub type Uid = String;

use base64_serde::base64_serde_type;

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSchema {
    pub username: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(with = "Base64Standard")]
    pub public_key: Vec<u8>,
    #[serde(with = "Base64Standard")]
    pub secret_key: Vec<u8>,
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

/// DataItemSummary
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
pub struct DataItemSummary {
    pub id: Id,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub owner: Uid,
    pub unique: Option<String>,
    pub parent_id: Option<String>,
}

impl salvo::Scribe for DataItemSummary {
    fn render(self, res: &mut salvo::Response) {
        res.render(salvo::writing::Json(self));
    }
}

impl From<DataItem> for DataItemSummary {
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

impl AccessLevel {
    pub fn to_string(&self) -> &'static str {
        match self {
            AccessLevel::Read => "read",
            AccessLevel::Update => "update",
            AccessLevel::Create => "create",
            AccessLevel::Write => "write",
            AccessLevel::FullAccess => "full_access",
        }
    }
}
impl std::str::FromStr for AccessLevel {
    type Err = StoreError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "read" => Ok(AccessLevel::Read),
            "update" => Ok(AccessLevel::Update),
            "create" => Ok(AccessLevel::Create),
            "write" => Ok(AccessLevel::Write),
            "full_access" => Ok(AccessLevel::FullAccess),
            _ => Err(StoreError::Validation(format!("Invalid access level string: {}", s))),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionSchema {
    pub data_id: String,
    pub user_id: String,
    pub access_level: AccessLevel,
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
