use crate::error::StoreResult;
use crate::types::{Id, Meta};
use serde_json::Value;

/// Minimal backend trait for storing JSON-like documents with meta.
pub trait Backend: Send + Sync {
    /// Insert a document (body) and associated meta. Returns the stored meta.
    fn insert(&self, collection: &str, body: &Value, meta: Meta) -> StoreResult<Meta>;

    /// Get a document by id.
    fn get(&self, collection: &str, id: &Id) -> StoreResult<(Value, Meta)>;

    /// Update an existing document by id. Returns updated meta.
    fn update(&self, collection: &str, id: &Id, body: &Value) -> StoreResult<Meta>;

    /// Delete a document by id.
    fn delete(&self, collection: &str, id: &Id) -> StoreResult<()>;
}

pub mod sqlite;

pub use sqlite::SqliteBackend;
