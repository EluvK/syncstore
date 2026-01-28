use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::rusqlite::{OptionalExtension, params};
use r2d2_sqlite::{SqliteConnectionManager, rusqlite};
use serde_json::Value;

use crate::backend::Backend;
use crate::error::{StoreError, StoreResult};
use crate::types::{AccessLevel, DataItem, DataItemDocument, Id, PermissionSchema};

// ?let's write some user define schema checker here for now, late move to separate file module.
mod checker {
    use std::sync::Arc;

    use jsonschema::Keyword;
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
        fn validate<'i>(&self, instance: &'i serde_json::Value) -> Result<(), jsonschema::ValidationError<'i>> {
            let msg_err = |msg: String| jsonschema::ValidationError::custom(msg);

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
    /// __acls: store access control list entries
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
                CREATE TABLE IF NOT EXISTS __acls (
                    id TEXT PRIMARY KEY,
                    data_collection TEXT NOT NULL,
                    data_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    permission TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    owner TEXT NOT NULL
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
        ) -> Result<Box<dyn jsonschema::Keyword>, Box<jsonschema::ValidationError<'a>>> {
            tracing::info!("more: value: {value:?}");
            tracing::info!("more: _parent: {:?}", _parent);
            let meta = serde_json::from_value(value.clone())
                .map_err(|e| jsonschema::ValidationError::custom(format!("x-parents: invalid meta format: {}", e)))?;
            tracing::info!("create parent check meta: {:?}", meta);
            Ok(Box::new(checker::XParentId {
                pool: pool.clone(),
                meta,
            }))
        }

        let compiled = jsonschema::draft7::options().with_keyword("x-parent-id", move |parent, value, path| {
            x_parent_id_check(parent, value, path, pool.clone()).map_err(|e| *e)
        });
        let compiled = compiled
            .build(schema)
            .map_err(|e| StoreError::Validation(format!("invalid schema: {}", e)))?;

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
    fn import(
        &self,
        collection: &str,
        body: &Value,
        owner: String,
        id: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> StoreResult<String> {
        self.validate_against_schema(collection, body)?;
        let body_text = serde_json::to_string(body)?;
        let table = sanitize_table_name(collection);
        let conn = self.get_conn()?;

        let unique = self.fetch_unique_field(collection, body)?;
        let parent_id = self.fetch_parent_id(collection, body)?;

        let sql = format!(
            "INSERT INTO {} (id, body, created_at, updated_at, owner, uniq, parent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            table
        );
        conn.execute(
            &sql,
            params![
                id,
                body_text,
                created_at.to_rfc3339(),
                updated_at.to_rfc3339(),
                owner,
                unique,
                parent_id
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
        Ok(id)
    }

    fn insert(&self, collection: &str, body: &Value, owner: String) -> StoreResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let created_at: chrono::DateTime<chrono::Utc> = now;
        let updated_at: chrono::DateTime<chrono::Utc> = now;
        self.import(collection, body, owner, id, created_at, updated_at)
    }

    fn list_by_owner(
        &self,
        collection: &str,
        owner: &str,
        marker: Option<String>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        let conn = self.get_conn()?;
        let table = sanitize_table_name(collection);
        // use a single query: if marker is NULL the WHERE clause is ignored
        let sql = format!(
            "SELECT id, body, created_at, updated_at, owner, uniq, parent_id \
             FROM {} \
             WHERE (owner = ?1) AND (?2 IS NULL OR id >= ?2) \
             ORDER BY id ASC \
             LIMIT ?3",
            table
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![owner, marker, limit as i64 + 1])?;
        let mut items = Vec::new();
        let mut next_marker: Option<String> = None;
        while let Some(row) = rows.next()? {
            let id = row.get::<_, String>(0)?;
            if items.len() == limit {
                // we have one more item, set next_marker
                next_marker = Some(id);
                break;
            }
            items.push(
                DataItemDocument {
                    id: id.clone(),
                    body: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    owner: row.get(4)?,
                    unique: row.get(5)?,
                    parent_id: row.get(6)?,
                }
                .try_into()?,
            );
        }
        Ok((items, next_marker))
    }

    fn list_children(
        &self,
        collection: &str,
        parent_id: &str,
        marker: Option<String>,
        limit: usize,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        let conn = self.get_conn()?;
        let table = sanitize_table_name(collection);
        // use a single query: if marker is NULL the WHERE clause is ignored
        let sql = format!(
            "SELECT id, body, created_at, updated_at, owner, uniq, parent_id \
             FROM {} \
             WHERE (parent_id = ?1) AND (?2 IS NULL OR id >= ?2) \
             ORDER BY id ASC \
             LIMIT ?3",
            table
        );
        // tracing::info!("list sql: {}, {}", sql, limit);
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![parent_id, marker, limit as i64 + 1])?;
        let mut items = Vec::new();
        let mut next_marker: Option<String> = None;
        while let Some(row) = rows.next()? {
            let id = row.get::<_, String>(0)?;
            if items.len() == limit {
                // we have one more item, set next_marker
                next_marker = Some(id);
                break;
            }
            items.push(
                DataItemDocument {
                    id: id.clone(),
                    body: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    owner: row.get(4)?,
                    unique: row.get(5)?,
                    parent_id: row.get(6)?,
                }
                .try_into()?,
            );
        }
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
        let data = stmt
            .query_row(params![id], |r| {
                Ok(DataItemDocument {
                    id: id.to_string(),
                    body: r.get(0)?,
                    created_at: r.get(1)?,
                    updated_at: r.get(2)?,
                    owner: r.get(3)?,
                    unique: r.get(4)?,
                    parent_id: r.get(5)?,
                })
            })
            .optional()?
            .ok_or(StoreError::NotFound(format!("Get Data {} / {}", collection, id)))?;
        data.try_into()
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
        let data = stmt
            .query_row(params![unique], |r| {
                Ok(DataItemDocument {
                    id: r.get(0)?,
                    body: r.get(1)?,
                    created_at: r.get(2)?,
                    updated_at: r.get(3)?,
                    owner: r.get(4)?,
                    unique: Some(unique.to_string()),
                    parent_id: r.get(5)?,
                })
            })
            .optional()?
            .ok_or(StoreError::NotFound("Get Data by Unique".to_string()))?;
        data.try_into()
    }

    fn update(&self, collection: &str, id: &Id, body: &Value) -> StoreResult<DataItem> {
        // validate data, ensure collection table exists and schema validated
        self.validate_against_schema(collection, body)?;
        let body_text = serde_json::to_string(body)?;
        let updated_at = chrono::Utc::now();
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

        // read back
        let item = self.get(collection, id)?;
        Ok(item)
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

    fn batch_delete(&self, collection: &str, ids: &[Id]) -> StoreResult<()> {
        let table = sanitize_table_name(collection);
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let sql = format!("DELETE FROM {} WHERE id = ?1", table);
        {
            let mut stmt = tx.prepare(&sql)?;
            for id in ids {
                let n = stmt.execute(params![id])?;
                if n == 0 {
                    return Err(StoreError::NotFound(format!("Delete Data id={}", id)));
                }
            }
            // drop stmt before commit
        }
        tx.commit()?;
        Ok(())
    }
}

// impl acls related methods
impl SqliteBackend {
    pub fn get_data_permissions(&self, data_collection: &str, data_id: &str) -> StoreResult<Vec<PermissionSchema>> {
        let conn = self.get_conn()?;
        let sql = "SELECT user_id, permission FROM __acls WHERE data_collection = ?1 AND data_id = ?2".to_string();
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![data_collection, data_id])?;
        let mut permissions = Vec::new();
        while let Some(row) = rows.next()? {
            let user_id: String = row.get(0)?;
            let permission_str: String = row.get(1)?;
            let access_level = AccessLevel::from_str(&permission_str)?;
            permissions.push(PermissionSchema {
                data_id: data_id.to_string(),
                user_id,
                access_level,
            });
        }
        Ok(permissions)
    }

    pub fn get_user_permissions(&self, data_collection: &str, user_id: &str) -> StoreResult<Vec<PermissionSchema>> {
        let conn = self.get_conn()?;
        let sql = "SELECT data_id, permission FROM __acls WHERE data_collection = ?1 AND user_id = ?2".to_string();
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![data_collection, user_id])?;
        let mut permissions = Vec::new();
        while let Some(row) = rows.next()? {
            let data_id: String = row.get(0)?;
            let permission_str: String = row.get(1)?;
            let access_level = AccessLevel::from_str(&permission_str)?;
            permissions.push(PermissionSchema {
                data_id,
                user_id: user_id.to_string(),
                access_level,
            });
        }
        Ok(permissions)
    }

    pub fn delete_acls_by_data_id(&self, data_collection: &str, data_id: &str) -> StoreResult<()> {
        let conn = self.get_conn()?;
        let sql = "DELETE FROM __acls WHERE data_collection = ?1 AND data_id = ?2".to_string();
        conn.execute(&sql, params![data_collection, data_id])?;
        Ok(())
    }

    pub fn update_acls(
        &self,
        data_collection: &str,
        data_id: &str,
        new_permissions: &[PermissionSchema],
        owner: &str,
    ) -> StoreResult<()> {
        let old_permissions = self.get_data_permissions(data_collection, data_id)?;

        let mut deleted_ids = Vec::new();
        let mut to_update_permissions = Vec::new();
        let mut new_permissions: HashMap<String, PermissionSchema> =
            HashMap::from_iter(new_permissions.iter().map(|p| (p.user_id.clone(), p.clone())));
        for old in &old_permissions {
            match new_permissions.remove(&old.user_id) {
                // exists in both old and new, update if different
                Some(new_p) if new_p.access_level != old.access_level => to_update_permissions.push(new_p),
                // same permission, do nothing
                Some(_) => {}
                // only in old, delete
                None => deleted_ids.push(old.user_id.clone()),
            }
        }
        let updated_at = chrono::Utc::now();
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        for user_id in deleted_ids {
            let sql = "DELETE FROM __acls WHERE data_collection = ?1 AND data_id = ?2 AND user_id = ?3".to_string();
            tx.execute(&sql, params![data_collection, data_id, user_id])?;
        }
        for p in to_update_permissions {
            let permission_str = p.access_level.to_string();
            let sql = "UPDATE __acls SET permission = ?1, updated_at = ?2 WHERE data_collection = ?3 AND data_id = ?4 AND user_id = ?5".to_string();
            tx.execute(
                &sql,
                params![
                    permission_str,
                    updated_at.to_rfc3339(),
                    data_collection,
                    data_id,
                    p.user_id
                ],
            )?;
        }
        for (_user_id, p) in new_permissions {
            let permission_str = p.access_level.to_string();
            let now = chrono::Utc::now();
            let sql = "INSERT INTO __acls (id, data_collection, data_id, user_id, permission, created_at, updated_at, owner) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)".to_string();
            let acl_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                &sql,
                params![
                    acl_id,
                    data_collection,
                    data_id,
                    p.user_id,
                    permission_str,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    owner
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }
}
