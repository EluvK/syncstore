use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::rusqlite::{OptionalExtension, params};
use r2d2_sqlite::{SqliteConnectionManager, rusqlite};
use serde_json::Value;

use crate::backend::Backend;
use crate::error::{StoreError, StoreResult};
use crate::types::{Id, Meta};

// ?let's write some user define schema checker here for now, late move to separate file module.
mod checker {
    use std::sync::Arc;

    use jsonschema::{Keyword, paths::Location};
    use r2d2::Pool;
    use r2d2_sqlite::{
        SqliteConnectionManager,
        rusqlite::{OptionalExtension, params},
    };

    pub struct DBExists {
        pub pool: Arc<Pool<SqliteConnectionManager>>,
        pub collection: String,
        pub column: String,
    }

    impl Keyword for DBExists {
        fn validate<'i>(
            &self,
            instance: &'i serde_json::Value,
            location: &jsonschema::paths::LazyLocation,
        ) -> Result<(), jsonschema::ValidationError<'i>> {
            let location: Location = (&location.clone()).into();
            let Some(value) = instance.as_str() else {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location.clone(),
                    instance,
                    "db_exists: expected string",
                ));
            };

            let Ok(conn) = self.pool.get() else {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location.clone(),
                    instance,
                    "db_exists: failed to get db connection",
                ));
            };

            let sql = format!("SELECT 1 FROM {} WHERE {} = ?1 LIMIT 1", self.collection, self.column);
            // println!("db_exists check sql: {}", sql);
            let exists = conn
                .query_row(&sql, params![value], |_| Ok(()))
                .optional()
                .map_err(|e| {
                    jsonschema::ValidationError::custom(
                        location.clone(),
                        location.clone(),
                        instance,
                        &format!("db_exists: db query error: {}", e),
                    )
                })?
                .is_some();

            if !exists {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location,
                    instance,
                    &format!(
                        "db_exists: value '{}' not found in {}.{}",
                        value, self.collection, self.column
                    ),
                ));
            }

            Ok(())
        }

        fn is_valid(&self, instance: &serde_json::Value) -> bool {
            let sql = format!("SELECT 1 FROM {} WHERE {} = ?1 LIMIT 1", self.collection, self.column);
            // println!("db_exists check sql: {}", sql);
            if let Some(value) = instance.as_str()
                && let Ok(conn) = self.pool.get()
                && let Ok(Some(_)) = conn.query_row(&sql, params![value], |_| Ok(())).optional()
            {
                return true;
            }
            false
        }
    }
}

/// Builder to create a SqliteBackend with options.
///
/// 1. first use `SqliteBackendBuilder::memory()` or `SqliteBackendBuilder::file(path)`
/// 2. then optionally call `with_collection_schema` to register each collection schemas,
/// 3. finally call `build()` to get the backend instance.
pub struct SqliteBackendBuilder {
    path: Option<PathBuf>,                    // if None, use in-memory database
    collection_schemas: Vec<(String, Value)>, // (collection name, json schema)
}

impl SqliteBackendBuilder {
    pub fn memory() -> Self {
        Self {
            path: None,
            collection_schemas: Vec::new(),
        }
    }
    pub fn file<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            collection_schemas: Vec::new(),
        }
    }

    pub fn with_collection_schema(mut self, collection: &str, schema: Value) -> Self {
        // todo should we check the schema is valid json schema? then the return type should be Result<Self, Error>
        // but we might need to crash as it is static config error then.? TBD
        self.collection_schemas.push((collection.to_string(), schema));
        self
    }
    pub fn build(self) -> StoreResult<SqliteBackend> {
        let mut backend = if let Some(p) = self.path {
            SqliteBackend::open(p)?
        } else {
            SqliteBackend::memory()?
        };
        // set collection schemas
        for (collection, schema) in self.collection_schemas {
            backend.init_collection_schema(&collection, &schema)?;
        }
        Ok(backend)
    }
}

/// One sqlite backend handle one certain database (file or memory)
/// Each database may contain multiple collections (tables).
/// Each collection do have its own JSON schema (stored in __schemas table).
///
/// User `SqliteBackendBuilder` to create an instance.
pub struct SqliteBackend {
    pool: Arc<Pool<SqliteConnectionManager>>,
    schema_validator: HashMap<String, jsonschema::Validator>,

    unique_fields: HashMap<String, String>, // collection -> unique field
}

impl SqliteBackend {
    // shared connection pool
    pub(crate) fn pool(&self) -> Arc<Pool<SqliteConnectionManager>> {
        self.pool.clone()
    }
    // in-memory sqlite
    fn memory() -> StoreResult<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager).map_err(|e| StoreError::Backend(e.to_string()))?;
        let backend = Self {
            pool: Arc::new(pool),
            schema_validator: HashMap::new(),
            unique_fields: HashMap::new(),
        };
        backend.init().map(|_| backend)
    }

    // file-based sqlite
    fn open<P: AsRef<Path>>(path: P) -> StoreResult<Self> {
        let manager = SqliteConnectionManager::file(path.as_ref());
        let pool = Pool::new(manager).map_err(|e| StoreError::Backend(e.to_string()))?;
        let backend = Self {
            pool: Arc::new(pool),
            schema_validator: HashMap::new(),
            unique_fields: HashMap::new(),
        };
        backend.init().map(|_| backend)
    }

    fn get_conn(&self) -> StoreResult<PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| StoreError::Backend(e.to_string()))
    }

    /// common initialization, create internal tables
    ///
    /// __schemas: store collection schemas
    ///
    fn init(&self) -> StoreResult<()> {
        // table to store collection schemas and a small meta for collections
        let conn = self.get_conn()?;
        conn.execute_batch(
            r#"
                CREATE TABLE IF NOT EXISTS __schemas (
                    collection TEXT PRIMARY KEY,
                    schema TEXT NOT NULL
                );
            "#,
        )
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(())
    }

    /// Save or update a collection schema.
    fn init_collection_schema(&mut self, collection: &str, schema: &Value) -> StoreResult<()> {
        let s = serde_json::to_string(schema).map_err(|e| StoreError::Backend(e.to_string()))?;
        let mut conn = self.get_conn()?;

        let tx = conn.transaction().map_err(|e| StoreError::Backend(e.to_string()))?;

        tx.execute(
            "INSERT INTO __schemas(collection, schema) VALUES (?1, ?2) ON CONFLICT(collection) DO UPDATE SET schema = excluded.schema",
            params![collection, s],
        )
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        // compile and cache the schema validator
        let pool = self.pool.clone();
        fn db_exists_func<'a>(
            _: &'a serde_json::Map<String, Value>,
            value: &'a Value,
            _: jsonschema::paths::Location,
            pool: Arc<Pool<SqliteConnectionManager>>,
        ) -> Result<Box<dyn jsonschema::Keyword>, jsonschema::ValidationError<'a>> {
            let collection = value["collection"].as_str().unwrap_or("").to_string();
            let column = value["column"].as_str().unwrap_or("").to_string();
            Ok(Box::new(checker::DBExists {
                pool: pool.clone(),
                collection,
                column,
            }))
        }
        let compiled = jsonschema::draft7::options().with_keyword("db_exists", move |parent, value, path| {
            db_exists_func(parent, value, path, pool.clone())
        });
        let compiled = compiled
            .build(schema)
            .map_err(|e| StoreError::Validation(format!("invalid schema: {}", e)))?;

        // let compiled =
        //     jsonschema::draft7::new(schema).map_err(|e| StoreError::Validation(format!("invalid schema: {}", e)))?;
        self.schema_validator.insert(collection.to_string(), compiled);
        // record the unique field if any
        if let Some(xu) = schema.get("x-unique").and_then(|v| v.as_str())
            && !xu.is_empty()
        {
            self.unique_fields.insert(collection.to_string(), xu.to_string());
        }

        // ensure collection table exists
        let table = sanitize_table_name(collection);

        // todo how to make `owner` db_exists to users.id?,
        //?actually it might be unnecessary as owner should be checked by auth module before coming here.
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                owner TEXT NOT NULL,
                uniq TEXT UNIQUE
            );",
            table
        );
        tx.execute_batch(&sql).map_err(|e| StoreError::Backend(e.to_string()))?;
        tx.commit().map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(())
    }

    // fetch the unique field value from body if was defined in schema
    fn fetch_unique_field(&self, collection: &str, body: &Value) -> StoreResult<Option<String>> {
        // todo future support nested field like "a.b.c"
        if let Some(field) = self.unique_fields.get(collection)
            && let Some(v) = body.get(field)
        {
            return match v.as_str() {
                Some(s) => Ok(Some(s.to_string())),
                None => serde_json::to_string(v)
                    .map(Some)
                    .map_err(|e| StoreError::Backend(e.to_string())),
            };
        }
        Ok(None)
    }

    fn validate_against_schema(&self, collection: &str, body: &Value) -> StoreResult<()> {
        self.schema_validator
            .get(collection)
            .ok_or_else(|| StoreError::Validation(format!("collection '{}' not registered", collection)))?
            .validate(body)
            .map_err(|errors| StoreError::Validation(errors.to_string()))?;
        Ok(())
    }
}

fn sanitize_table_name(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    // prefix to avoid collision with internal tables
    format!("c_{}", s)
}

impl Backend for SqliteBackend {
    fn insert(&self, collection: &str, body: &Value, meta: Meta) -> StoreResult<Meta> {
        // validate data, ensure collection table exists and schema validated
        self.validate_against_schema(collection, body)?;
        let body_text = serde_json::to_string(body).map_err(|e| StoreError::Backend(e.to_string()))?;
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;

        let mut meta = meta;
        if meta.unique.is_none() {
            meta.unique = self.fetch_unique_field(collection, body)?;
        }

        let sql = format!(
            "INSERT INTO {} (id, body, created_at, updated_at, owner, uniq) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            table
        );
        conn.execute(
            &sql,
            params![
                meta.id,
                body_text,
                meta.created_at.to_rfc3339(),
                meta.updated_at.to_rfc3339(),
                meta.owner,
                meta.unique
            ],
        )
        .map_err(|e| match &e {
            rusqlite::Error::SqliteFailure(err, msg)
                if err.code == rusqlite::ErrorCode::ConstraintViolation
                    && msg.as_ref().is_some_and(|m| m.contains("UNIQUE")) =>
            {
                StoreError::Validation(format!("unique constraint violation: {}, {:?}", err, msg))
            }
            rusqlite::Error::SqliteFailure(err, msg) if err.code == rusqlite::ErrorCode::ConstraintViolation => {
                StoreError::Validation(format!("id already exists: {}, {:?}", err, msg))
            }
            _ => StoreError::Backend(e.to_string()),
        })?;
        Ok(meta)
    }

    fn get(&self, collection: &str, id: &Id) -> StoreResult<(Value, Meta)> {
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!(
            "SELECT body, created_at, updated_at, owner, uniq FROM {} WHERE id = ?1",
            table
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| StoreError::Backend(e.to_string()))?;

        let row = stmt
            .query_row(params![id], |r| {
                let body_text: String = r.get(0)?;
                let created_at: String = r.get(1)?;
                let updated_at: String = r.get(2)?;
                let owner: String = r.get(3)?;
                let unique: Option<String> = r.get(4)?;
                Ok((body_text, created_at, updated_at, owner, unique))
            })
            .optional()
            .map_err(|e| StoreError::Backend(e.to_string()))?;

        if let Some((body_text, created_at, updated_at, owner, unique)) = row {
            let body: Value = serde_json::from_str(&body_text).map_err(|e| StoreError::Backend(e.to_string()))?;
            let meta = Meta {
                id: id.clone(),
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| StoreError::Backend(e.to_string()))?
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| StoreError::Backend(e.to_string()))?
                    .with_timezone(&chrono::Utc),
                owner,
                unique,
            };
            Ok((body, meta))
        } else {
            Err(StoreError::NotFound)
        }
    }

    fn get_by_unique(&self, collection: &str, unique: &str) -> StoreResult<(Value, Meta)> {
        if !self.unique_fields.contains_key(collection) {
            return Err(StoreError::Validation(format!(
                "collection '{}' does not have unique field defined",
                collection
            )));
        }
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!(
            "SELECT id, body, created_at, updated_at, owner FROM {} WHERE uniq = ?1",
            table
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| StoreError::Backend(e.to_string()))?;
        let row = stmt
            .query_row(params![unique], |r| {
                let id: String = r.get(0)?;
                let body_text: String = r.get(1)?;
                let created_at: String = r.get(2)?;
                let updated_at: String = r.get(3)?;
                let owner: String = r.get(4)?;
                Ok((id, body_text, created_at, updated_at, owner))
            })
            .optional()
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        if let Some((id, body_text, created_at, updated_at, owner)) = row {
            let body: Value = serde_json::from_str(&body_text).map_err(|e| StoreError::Backend(e.to_string()))?;
            let meta = Meta {
                id,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| StoreError::Backend(e.to_string()))?
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| StoreError::Backend(e.to_string()))?
                    .with_timezone(&chrono::Utc),
                owner,
                unique: Some(unique.to_string()),
            };
            Ok((body, meta))
        } else {
            Err(StoreError::NotFound)
        }
    }

    fn update(&self, collection: &str, id: &Id, body: &Value) -> StoreResult<Meta> {
        // validate data, ensure collection table exists and schema validated
        self.validate_against_schema(collection, body)?;
        let body_text = serde_json::to_string(body).map_err(|e| StoreError::Backend(e.to_string()))?;
        let updated_at = chrono::Utc::now().to_rfc3339();
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let unique = self.fetch_unique_field(collection, body)?;
        let sql = format!(
            "UPDATE {} SET body = ?1, updated_at = ?2, uniq = ?3 WHERE id = ?4",
            table
        );
        let n = conn
            .execute(&sql, params![body_text, updated_at, unique, id])
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }

        // read back meta
        let (_body, meta) = self.get(collection, id)?;
        Ok(meta)
    }

    fn delete(&self, collection: &str, id: &Id) -> StoreResult<()> {
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!("DELETE FROM {} WHERE id = ?1", table);
        let n = conn
            .execute(&sql, params![id])
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        if n == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
