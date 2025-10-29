use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::rusqlite::{OptionalExtension, params};
use r2d2_sqlite::{SqliteConnectionManager, rusqlite};
use serde_json::Value;

use crate::backend::Backend;
use crate::error::{StoreError, StoreResult};
use crate::types::{DataItem, Id, Meta};

// ?let's write some user define schema checker here for now, late move to separate file module.
mod checker {
    use std::sync::Arc;

    use jsonschema::{Keyword, paths::Location};
    use r2d2::Pool;
    use r2d2_sqlite::{
        SqliteConnectionManager,
        rusqlite::{OptionalExtension, params},
    };
    use serde::Deserialize;

    use crate::backend::sqlite::sanitize_table_name;

    #[derive(Debug, Clone, Deserialize)]
    pub struct XParentIdMeta {
        pub parent: String,
        pub field: String,
    }

    pub struct XParentId {
        pub pool: Arc<Pool<SqliteConnectionManager>>,
        pub meta: XParentIdMeta,
    }

    impl Keyword for XParentId {
        fn validate<'i>(
            &self,
            instance: &'i serde_json::Value,
            location: &jsonschema::paths::LazyLocation,
        ) -> Result<(), jsonschema::ValidationError<'i>> {
            let location: Location = (&location.clone()).into();

            let msg_err =
                |msg: String| jsonschema::ValidationError::custom(location.clone(), location.clone(), instance, msg);

            tracing::info!(
                "x_parent[validate] current self addr {:?} current instance: {:?}",
                std::ptr::addr_of!(self),
                instance
            );
            let m = &self.meta;
            tracing::info!("x_parent[validate] check meta: {:?}", m);
            let Some(value) = instance.get(&m.field).and_then(|f| f.as_str()) else {
                return Err(msg_err("x_parent: field value missing or not string".into()));
            };
            let Ok(conn) = self.pool.get() else {
                return Err(msg_err("x_parent: failed to get db connection".into()));
            };
            let sql = format!(
                "SELECT body, owner FROM {} WHERE id = ?1 LIMIT 1",
                sanitize_table_name(&m.parent),
            );
            let data = conn
                .query_row(&sql, params![value], |r| {
                    let body_text: String = r.get(0)?;
                    let owner: String = r.get(1)?;
                    Ok((body_text, owner))
                })
                .optional()
                .map_err(|e| msg_err(format!("x_parent: db query error: {}", e)))?;
            let Some((body_text, parent_owner)) = data else {
                return Err(msg_err(format!(
                    "x_parent: parent id '{}' not found in {}",
                    value, m.parent
                )));
            };
            tracing::info!(
                "x_parent found parent record: body={}, owner={}",
                body_text,
                parent_owner
            );
            let _body: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| msg_err(e.to_string()))?;
            Ok(())
        }

        fn is_valid(&self, instance: &serde_json::Value) -> bool {
            let m = &self.meta;
            tracing::info!("x_parent[is_valid] check meta: {:?}", m);
            let sql = format!(
                "SELECT body, owner FROM {} WHERE id = ?1 LIMIT 1",
                sanitize_table_name(&m.parent)
            );
            if let Some(value) = instance.get(&m.field).and_then(|f| f.as_str())
                && let Ok(conn) = self.pool.get()
                && let Ok(Some((_body_text, _parent_owner))) = conn
                    .query_row(&sql, params![value], |r| {
                        let body_text: String = r.get(0)?;
                        let owner: String = r.get(1)?;
                        Ok((body_text, owner))
                    })
                    .optional()
            // && let Ok(body) = serde_json::from_str::<serde_json::Value>(&body_text)
            // && let Some(field_value) = body.get(&m.field).and_then(|f| f.as_str())
            {
                // if self.owner_match {
                //     return self.owner == parent_owner;
                // }
                // return true;
            } else {
                return false;
            }
            true
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
    // every collection's compiled schema validator
    schema_validator: HashMap<String, jsonschema::Validator>,

    // every collection's parent collection info
    parent_ref: HashMap<String, checker::XParentIdMeta>,
    unique_fields: HashMap<String, String>, // collection -> unique field
}

impl SqliteBackend {
    // return parent collection name and parent field name in current data item key
    pub(crate) fn parent_collection(&self, collection: &str) -> Option<(&str, &str)> {
        self.parent_ref
            .get(collection)
            .map(|m| (m.parent.as_str(), m.field.as_str()))
    }

    fn new(pool: Arc<Pool<SqliteConnectionManager>>) -> Self {
        Self {
            pool,
            schema_validator: HashMap::new(),
            parent_ref: HashMap::new(),
            unique_fields: HashMap::new(),
        }
    }

    // in-memory sqlite
    fn memory() -> StoreResult<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;
        let backend = Self::new(Arc::new(pool));
        backend.init().map(|_| backend)
    }

    // file-based sqlite
    fn open<P: AsRef<Path>>(path: P) -> StoreResult<Self> {
        let manager = SqliteConnectionManager::file(path.as_ref());
        let pool = Pool::new(manager)?;
        let backend = Self::new(Arc::new(pool));
        backend.init().map(|_| backend)
    }

    fn get_conn(&self) -> StoreResult<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
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
        )?;
        Ok(())
    }

    /// Save or update a collection schema.
    fn init_collection_schema(&mut self, collection: &str, schema: &Value) -> StoreResult<()> {
        let s = serde_json::to_string(schema)?;
        let mut conn = self.get_conn()?;

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO __schemas(collection, schema) VALUES (?1, ?2) ON CONFLICT(collection) DO UPDATE SET schema = excluded.schema",
            params![collection, s],
        )
        ?;
        // compile and cache the schema validator
        let pool = self.pool.clone();

        fn x_parent_id_check<'a>(
            _parent: &'a serde_json::Map<String, Value>,
            value: &'a Value,
            _path: jsonschema::paths::Location,
            pool: Arc<Pool<SqliteConnectionManager>>,
        ) -> Result<Box<dyn jsonschema::Keyword>, jsonschema::ValidationError<'a>> {
            tracing::info!("more: value: {value:?}");
            tracing::info!("more: _parent: {:?}", _parent);
            let meta = serde_json::from_value(value.clone()).map_err(|e| {
                jsonschema::ValidationError::custom(
                    _path.clone(),
                    _path.clone(),
                    value,
                    format!("x-parents: invalid meta format: {}", e),
                )
            })?;
            tracing::info!("create parent check meta: {:?}", meta);
            Ok(Box::new(checker::XParentId {
                pool: pool.clone(),
                meta,
            }))
        }

        let compiled = jsonschema::draft7::options()
            // .with_keyword("db_exists", move |parent, value, path| {
            //     db_exists_func(parent, value, path, pool.clone())
            // })
            .with_keyword("x-parent-id", move |parent, value, path| {
                x_parent_id_check(parent, value, path, pool.clone())
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
        if let Some(xpi) = schema
            .get("x-parent-id")
            .and_then(|v| serde_json::from_value::<checker::XParentIdMeta>(v.clone()).ok())
        {
            tracing::info!("init_collection_schema x-parent-id: {:?}", xpi);
            self.parent_ref.insert(collection.to_string(), xpi);
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
                uniq TEXT UNIQUE,
                parent_id TEXT
            );",
            table
        );
        tx.execute_batch(&sql)?;
        tx.commit()?;
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
                None => Ok(Some(serde_json::to_string(v)?)),
            };
        }
        Ok(None)
    }

    fn fetch_parent_id(&self, collection: &str, body: &Value) -> StoreResult<Option<String>> {
        if let Some(xpm) = self.parent_ref.get(collection)
            && let Some(v) = body.get(&xpm.field)
        {
            return match v.as_str() {
                Some(s) => Ok(Some(s.to_string())),
                None => Ok(Some(serde_json::to_string(v)?)),
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
        let body_text = serde_json::to_string(body)?;
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;

        let mut meta = meta;
        if meta.unique.is_none() {
            meta.unique = self.fetch_unique_field(collection, body)?;
        }
        if meta.parent_id.is_none() {
            meta.parent_id = self.fetch_parent_id(collection, body)?;
        }

        let sql = format!(
            "INSERT INTO {} (id, body, created_at, updated_at, owner, uniq, parent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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
                meta.unique,
                meta.parent_id
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

    fn list(
        &self,
        collection: &str,
        parent_id: &str,
        marker: Option<&str>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        let conn = self.get_conn()?;
        let table = sanitize_table_name(collection);
        // use a single query: if marker is NULL the WHERE clause is ignored
        let sql = format!(
            "SELECT id, body, created_at, updated_at, owner, uniq, parent_id \
             FROM {} \
             WHERE (parent_id = ?1) AND (?2 IS NULL OR id > ?2) \
             ORDER BY id ASC \
             LIMIT ?3",
            table
        );
        tracing::info!("list sql: {}, {}", sql, limit);
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![parent_id, marker, limit as i64])?;
        let mut items = Vec::new();
        let mut last_id: Option<String> = None;
        while let Some(row) = rows.next()? {
            let id = row.get::<_, String>(0)?;
            items.push(DataItem {
                id: id.clone(),
                body: serde_json::from_str::<Value>(&row.get::<_, String>(1)?)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)?
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)?
                    .with_timezone(&chrono::Utc),
                owner: row.get(4)?,
                unique: row.get(5)?,
                parent_id: row.get(6)?,
            });
            last_id = Some(id);
        }
        let next_marker = if items.len() == limit { last_id } else { None };
        Ok((items, next_marker))
    }

    fn get(&self, collection: &str, id: &Id) -> StoreResult<DataItem> {
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!(
            "SELECT body, created_at, updated_at, owner, uniq, parent_id FROM {} WHERE id = ?1",
            table
        );
        let mut stmt = conn.prepare(&sql)?;

        let row = stmt
            .query_row(params![id], |r| {
                let body_text: String = r.get(0)?;
                let created_at: String = r.get(1)?;
                let updated_at: String = r.get(2)?;
                let owner: String = r.get(3)?;
                let unique: Option<String> = r.get(4)?;
                let parent_id: Option<String> = r.get(5)?;
                Ok((body_text, created_at, updated_at, owner, unique, parent_id))
            })
            .optional()?;

        if let Some((body_text, created_at, updated_at, owner, unique, parent_id)) = row {
            let body: Value = serde_json::from_str(&body_text)?;

            Ok(DataItem {
                id: id.to_string(),
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&chrono::Utc),
                owner,
                unique,
                parent_id,
                body,
            })
        } else {
            Err(StoreError::NotFound(format!("Get Data {} / {}", collection, id)))
        }
    }

    fn get_by_unique(&self, collection: &str, unique: &str) -> StoreResult<DataItem> {
        if !self.unique_fields.contains_key(collection) {
            return Err(StoreError::Validation(format!(
                "collection '{}' does not have unique field defined",
                collection
            )));
        }
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!(
            "SELECT id, body, created_at, updated_at, owner, parent_id FROM {} WHERE uniq = ?1",
            table
        );
        let mut stmt = conn.prepare(&sql)?;
        let row = stmt
            .query_row(params![unique], |r| {
                let id: String = r.get(0)?;
                let body_text: String = r.get(1)?;
                let created_at: String = r.get(2)?;
                let updated_at: String = r.get(3)?;
                let owner: String = r.get(4)?;
                let parent_id: Option<String> = r.get(5)?;
                Ok((id, body_text, created_at, updated_at, owner, parent_id))
            })
            .optional()?;
        if let Some((id, body_text, created_at, updated_at, owner, parent_id)) = row {
            let body: Value = serde_json::from_str(&body_text)?;
            Ok(DataItem {
                id,
                body,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)?.with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)?.with_timezone(&chrono::Utc),
                owner,
                unique: Some(unique.to_string()),
                parent_id,
            })
        } else {
            Err(StoreError::NotFound("Get Data by Unique".to_string()))
        }
    }

    fn update(&self, collection: &str, id: &Id, body: &Value) -> StoreResult<Meta> {
        // validate data, ensure collection table exists and schema validated
        self.validate_against_schema(collection, body)?;
        let body_text = serde_json::to_string(body)?;
        let updated_at = chrono::Utc::now().to_rfc3339();
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let unique = self.fetch_unique_field(collection, body)?;
        let parent_id = self.fetch_parent_id(collection, body)?;
        let sql = format!(
            "UPDATE {} SET body = ?1, updated_at = ?2, uniq = ?3, parent_id = ?4 WHERE id = ?5",
            table
        );
        let n = conn.execute(&sql, params![body_text, updated_at, unique, parent_id, id])?;
        if n == 0 {
            return Err(StoreError::NotFound("Update Data".to_string()));
        }

        // read back meta
        let item = self.get(collection, id)?;
        Ok(item.into())
    }

    fn delete(&self, collection: &str, id: &Id) -> StoreResult<()> {
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;
        let sql = format!("DELETE FROM {} WHERE id = ?1", table);
        let n = conn.execute(&sql, params![id])?;
        if n == 0 {
            return Err(StoreError::NotFound("Delete Data".to_string()));
        }
        Ok(())
    }
}
