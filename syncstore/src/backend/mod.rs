use crate::error::StoreResult;
use crate::types::{DataItem, Id, Meta};
use serde_json::Value;

/// Minimal backend trait for storing JSON-like documents with meta.
pub trait Backend: Send + Sync {
    /// Insert a document (body) and associated meta. Returns the stored meta.
    fn insert(&self, collection: &str, body: &Value, meta: Meta) -> StoreResult<Meta>;

    /// List documents in a collection with pagination.
    fn list(
        &self,
        collection: &str,
        parent_id: Option<&str>,
        limit: usize,
        marker: Option<&str>,
        user: &str,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)>;

    /// Get a document by id.
    fn get(&self, collection: &str, id: &Id, user: &str) -> StoreResult<DataItem>;

    /// Get a document by unique field.
    fn get_by_unique(&self, collection: &str, unique: &str, user: &str) -> StoreResult<DataItem>;

    /// Update an existing document by id. Returns updated meta.
    fn update(&self, collection: &str, id: &Id, body: &Value, user: &str) -> StoreResult<Meta>;

    /// Delete a document by id.
    fn delete(&self, collection: &str, id: &Id, user: &str) -> StoreResult<()>;
}

pub mod sqlite;

pub use sqlite::SqliteBackend;
