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
    pub parent_id: Option<String>
    // should constructed from schema, only used in memory, not serialized to DB
    // #[serde(skip)]
    // pub references: Vec<Reference>,
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
    // todo maybe add a schema param later to construct references
    pub fn new(owner: Uid, unique: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            owner,
            unique,
            parent_id: None,
            // references: vec![],
        }
    }
}

/// Describe a reference from this record to another collection.field (refield is the referenced field).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    /// referenced field name in current collection.field
    pub refield: String,
    /// target collection name, e.g. "user"
    pub collection: String,
    /// target field name within collection, e.g. "id"
    pub field: String,
    /// whether the reference is a "belongs to" relationship, mostly true
    pub belongs: bool,
}

impl Reference {
    pub fn new<S>(refield: S, collection: S, field: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            collection: collection.into(),
            field: field.into(),
            refield: refield.into(),
            belongs: true,
        }
    }

    pub fn new_non_belongs<S>(refield: S, collection: S, field: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            collection: collection.into(),
            field: field.into(),
            refield: refield.into(),
            belongs: false,
        }
    }
}
