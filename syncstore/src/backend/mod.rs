use crate::error::StoreResult;
use crate::types::{DataItem, Id};
use serde_json::Value;

/// Minimal backend trait for storing JSON-like documents with meta.
pub trait Backend: Send + Sync {
    /// extra fields: created_at, updated_at, only for import existing data with specific timestamps
    /// for tools db_convert.rs
    fn import(
        &self,
        collection: &str,
        body: &Value,
        owner: String,
        id: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> StoreResult<String>;

    /// Insert a document into a collection. Returns the document id.
    fn insert(&self, collection: &str, body: &Value, owner: String) -> StoreResult<String>;

    /// List documents in a collection under certain owner with pagination
    fn list_by_owner(
        &self,
        collection: &str,
        owner: &str,
        marker: Option<String>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)>;

    /// List documents in a collection under certain parent's data with pagination
    fn list_children(
        &self,
        collection: &str,
        parent_id: &str,
        marker: Option<String>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)>;

    /// List documents in a collection with inspection field and pagination
    fn list_by_inspect(
        &self,
        collection: &str,
        inspect: &str,
        marker: Option<String>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)>;

    /// Get a document by id.
    fn get(&self, collection: &str, id: &Id) -> StoreResult<DataItem>;

    /// Get a document by unique field.
    fn get_by_unique(&self, collection: &str, unique: &str) -> StoreResult<DataItem>;

    /// Update an existing document by id
    fn update(&self, collection: &str, id: &Id, body: &Value) -> StoreResult<DataItem>;

    /// Delete a document by id.
    fn delete(&self, collection: &str, id: &Id) -> StoreResult<()>;

    /// Batch delete documents by ids.
    fn batch_delete(&self, collection: &str, ids: &[Id]) -> StoreResult<()>;
}

pub mod sqlite;

pub use sqlite::SqliteBackend;
