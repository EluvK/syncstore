use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    backend::{SqliteBackend, sqlite::SqliteBackendBuilder},
    error::{StoreError, StoreResult},
};

pub const MEMORY_NAMESPACE: &str = ":memory:";

/// A manager that holds sqlite backends per namespace (each namespace -> separate sqlite file).
/// Use `DataManagerBuilder` to create an instance.
#[derive(Clone, Default)]
pub struct DataManager {
    // dict<namespace, backend>
    map: HashMap<String, Arc<SqliteBackend>>,
    _base_dir: PathBuf,
}

impl DataManager {
    pub(crate) fn backend_for(&self, namespace: &str) -> StoreResult<Arc<SqliteBackend>> {
        match self.map.get(namespace) {
            Some(b) => Ok(b.clone()),
            None => Err(StoreError::NotFound(namespace.to_string())),
        }
    }
}

pub struct DataManagerBuilder {
    base_dir: PathBuf,
    map: HashMap<String, Arc<SqliteBackend>>,
}

impl DataManagerBuilder {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            map: HashMap::new(),
        }
    }

    pub fn add_memory_db(mut self, schemas: DataSchemas) -> StoreResult<Self> {
        let mut backend = SqliteBackendBuilder::memory();
        for (collection, schema) in schemas.map.into_iter() {
            backend = backend.with_collection_schema(&collection, schema);
        }
        let backend = backend.build()?;
        self.map.insert(MEMORY_NAMESPACE.into(), Arc::new(backend));

        Ok(self)
    }

    pub fn add_db(mut self, namespace: &str, schemas: DataSchemas) -> StoreResult<Self> {
        let mut path = self.base_dir.clone();
        std::fs::create_dir_all(&path)?;
        path.push(format!("{}.db", namespace));
        let mut backend = SqliteBackendBuilder::file(path);
        for (collection, schema) in schemas.map.into_iter() {
            backend = backend.with_collection_schema(&collection, schema);
        }
        let backend = backend.build()?;
        self.map.insert(namespace.to_string(), Arc::new(backend));
        Ok(self)
    }

    pub fn build(self) -> DataManager {
        DataManager {
            _base_dir: self.base_dir,
            map: self.map,
        }
    }
}

pub struct DataSchemas {
    // dict<collection, schema>
    map: HashMap<String, serde_json::Value>,
}

pub struct DataSchemasBuilder {
    map: HashMap<String, serde_json::Value>,
}

impl Default for DataSchemasBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSchemasBuilder {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn add_schema(mut self, collection: &str, schema: serde_json::Value) -> Self {
        self.map.insert(collection.to_string(), schema);
        self
    }

    pub fn build(self) -> DataSchemas {
        DataSchemas { map: self.map }
    }
}

/// # Example
/// ```rust
/// use syncstore::collection;
/// use syncstore::components::DataManagerBuilder;
/// use serde_json::json;
///
/// // Define schemas for collections
/// let schemas = collection! {
///     "posts" => json!({"type": "object", "properties": { "title": { "type": "string" } }, "required": ["title"] }),
/// };
///
/// // Build a DataManager with an in-memory database
/// let manager = DataManagerBuilder::new("/tmp/dbs")
///     .add_memory_db(schemas).unwrap()
///     .build();
///
/// // Access the backend for the in-memory namespace
/// let backend = manager.backend_for(":memory:").unwrap();
/// ```
#[macro_export]
macro_rules! collection {
    ( $( $collection:expr => $schema:expr ),* $(,)? ) => {{
        let mut builder = $crate::components::DataSchemasBuilder::new();
        $(
            builder = builder.add_schema($collection, $schema);
        )*
        builder.build()
    }};
}
