use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

/// Identifier type used across the store.
pub type Id = String;
pub type Uid = String;

use base64_serde::base64_serde_type;

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserSchemaDocument {
    pub username: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(with = "Base64Standard")]
    pub public_key: Vec<u8>,
    #[serde(with = "Base64Standard")]
    pub secret_key: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct UserSchema {
    pub user_id: String,
    pub username: String,
    pub password: String,
    pub avatar_url: Option<String>,
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>,
}

impl UserSchema {
    pub fn from_document(user_id: String, doc: UserSchemaDocument) -> Self {
        UserSchema {
            user_id,
            username: doc.username,
            password: doc.password,
            avatar_url: doc.avatar_url,
            public_key: doc.public_key,
            secret_key: doc.secret_key,
        }
    }
}

impl From<UserSchema> for UserSchemaDocument {
    fn from(value: UserSchema) -> Self {
        UserSchemaDocument {
            username: value.username,
            password: value.password,
            avatar_url: value.avatar_url,
            public_key: value.public_key,
            secret_key: value.secret_key,
        }
    }
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

/// This enum string will be stored in the database, so be sure to make compatible changes when modifying it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, salvo::oapi::ToSchema, salvo::oapi::ToResponse)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    /// Can only read existing data.
    Read,
    /// Can read and append data with parent-child relationship up to 1 level.
    ReadAppend1,
    /// Can read and append data with parent-child relationship up to 2 levels.
    ReadAppend2,
    /// Can read and append data with parent-child relationship up to 3 levels.
    ReadAppend3,
    /// Can read and update **existing** data(not really useful currently?).
    Update,
    /// Can read and create new data (anything but delete).
    Write,
    FullAccess,
}

impl AccessLevel {
    pub fn to_string(&self) -> &'static str {
        match self {
            AccessLevel::Read => "read",
            AccessLevel::ReadAppend1 => "read_append1",
            AccessLevel::ReadAppend2 => "read_append2",
            AccessLevel::ReadAppend3 => "read_append3",
            AccessLevel::Update => "update",
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
            "read_append1" => Ok(AccessLevel::ReadAppend1),
            "read_append2" => Ok(AccessLevel::ReadAppend2),
            "read_append3" => Ok(AccessLevel::ReadAppend3),
            "update" => Ok(AccessLevel::Update),
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
    // ACLMask used internally for permission checking, it corresponds to CRUD operations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ACLMask: u8 {
        const READ_ONLY      = 0b000001;
        const UPDATE_ONLY    = 0b000010;
        const DELETE_ONLY    = 0b000100;
        const APPEND_3_BELOW = 0b001000;
        const APPEND_2_BELOW = 0b011000;
        const APPEND_1_BELOW = 0b111000;
        const FULL_ACCESS    = 0b111111;
    }
}

impl ACLMask {
    pub fn upgrade_for_parent(self) -> Option<Self> {
        let current_append_bits = self & ACLMask::APPEND_1_BELOW;
        if current_append_bits.is_empty() {
            return Some(self);
        }
        let next_append_bits = match current_append_bits {
            ACLMask::APPEND_1_BELOW => Some(ACLMask::APPEND_2_BELOW),
            ACLMask::APPEND_2_BELOW => Some(ACLMask::APPEND_3_BELOW),
            ACLMask::APPEND_3_BELOW => None,
            // should not happen, as we already return Some(self) if no append bits
            _ => panic!("Invalid ACLMask for append levels"),
        };
        next_append_bits.map(|next_append_bits| (self - current_append_bits) | next_append_bits)
    }
}

impl From<AccessLevel> for ACLMask {
    fn from(level: AccessLevel) -> Self {
        match level {
            AccessLevel::Read => ACLMask::READ_ONLY,
            AccessLevel::ReadAppend1 => ACLMask::READ_ONLY | ACLMask::APPEND_1_BELOW,
            AccessLevel::ReadAppend2 => ACLMask::READ_ONLY | ACLMask::APPEND_2_BELOW,
            AccessLevel::ReadAppend3 => ACLMask::READ_ONLY | ACLMask::APPEND_3_BELOW,
            AccessLevel::Update => ACLMask::READ_ONLY | ACLMask::UPDATE_ONLY,
            AccessLevel::Write => ACLMask::READ_ONLY | ACLMask::UPDATE_ONLY | ACLMask::APPEND_1_BELOW,
            AccessLevel::FullAccess => ACLMask::FULL_ACCESS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_for_parent_progression() {
        let level1 = ACLMask::READ_ONLY | ACLMask::APPEND_1_BELOW;
        let level2 = level1.upgrade_for_parent().expect("Should transition to level 2");
        assert_eq!(level2, ACLMask::READ_ONLY | ACLMask::APPEND_2_BELOW);

        let level3 = level2.upgrade_for_parent().expect("Should transition to level 3");
        assert_eq!(level3, ACLMask::READ_ONLY | ACLMask::APPEND_3_BELOW);

        let level_none = level3.upgrade_for_parent();
        assert!(level_none.is_none(), "Level 3 should have no further levels");
    }

    #[test]
    fn test_upgrade_for_parent_preserves_other_bits() {
        let complex_mask = ACLMask::READ_ONLY | ACLMask::UPDATE_ONLY | ACLMask::APPEND_1_BELOW;
        let next = complex_mask.upgrade_for_parent().unwrap();

        assert!(next.contains(ACLMask::READ_ONLY));
        assert!(next.contains(ACLMask::UPDATE_ONLY));
        assert!(next.contains(ACLMask::APPEND_2_BELOW));
        assert!(!next.contains(ACLMask::APPEND_1_BELOW));
    }

    #[test]
    fn test_upgrade_for_parent_no_append_bits() {
        let read_only = ACLMask::READ_ONLY;
        let next = read_only.upgrade_for_parent().unwrap();
        assert_eq!(next, ACLMask::READ_ONLY);
    }
}
