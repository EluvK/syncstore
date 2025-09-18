use std::{path::Path, sync::Arc};

use crate::{
    backend::{Backend, SqliteBackend, sqlite::SqliteBackendBuilder},
    error::StoreResult,
    types::Meta,
};

pub struct UserManager {
    backend: Arc<SqliteBackend>,
}

// todo need to add a jwt auth for user management
// ? real hate to build more wheels, late to do this, after adding route for all db modules.
// ? for now a quick simple name password check.
impl UserManager {
    pub fn new(base_dir: impl AsRef<Path>) -> StoreResult<Self> {
        let mut path = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        path.push("users.db");

        let user_auth = serde_json::json!({
            "type": "object",
            "properties": {
                "username": { "type": "string" },
                "password": { "type": "string" }
            },
            "required": ["username", "password"]
        });
        let backend = Arc::new(
            SqliteBackendBuilder::file(path)
                .with_collection_schema("users", user_auth)
                .build()?,
        );

        Ok(UserManager { backend })
    }

    pub fn create_user(&self, username: &str, password: &str) -> StoreResult<()> {
        let user = serde_json::json!({
            "username": username,
            "password": password
        });
        // todo lots of works here...
        let meta = Meta::new("system".to_string());
        self.backend.insert("users", &user, meta)?;
        Ok(())
    }
}

mod checker {
    use std::sync::Arc;

    use jsonschema::{Keyword, paths::Location};
    use r2d2::Pool;
    use r2d2_sqlite::{
        SqliteConnectionManager,
        rusqlite::{OptionalExtension, params},
    };

    //? not sure if this check even necessary...
    pub struct UserExistChecker {
        pool: Arc<Pool<SqliteConnectionManager>>,
    }
    impl Keyword for UserExistChecker {
        fn validate<'i>(
            &self,
            instance: &'i serde_json::Value,
            location: &jsonschema::paths::LazyLocation,
        ) -> Result<(), jsonschema::ValidationError<'i>> {
            let location: Location = (&location.clone()).into();
            let Some(user_id) = instance.as_str() else {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location.clone(),
                    instance,
                    "user field should be string",
                ));
            };

            let Ok(conn) = self.pool.get() else {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location.clone(),
                    instance,
                    "fail to get user db connection",
                ));
            };

            let sql = format!("SELECT 1 FROM __users WHERE id = ?1 LIMIT 1");
            // println!("db_exists check sql: {}", sql);
            let exists = conn
                .query_row(&sql, params![user_id], |_| Ok(()))
                .optional()
                .map_err(|e| {
                    jsonschema::ValidationError::custom(
                        location.clone(),
                        location.clone(),
                        instance,
                        &format!("user exist db query error: {}", e),
                    )
                })?
                .is_some();

            if !exists {
                return Err(jsonschema::ValidationError::custom(
                    location.clone(),
                    location,
                    instance,
                    &format!("user id '{}' does not exist in users collection", user_id),
                ));
            }

            Ok(())
        }

        fn is_valid(&self, instance: &serde_json::Value) -> bool {
            let sql = format!("SELECT 1 FROM __users WHERE id = ?1 LIMIT 1");
            if let Some(user_id) = instance.as_str()
                && let Ok(conn) = self.pool.get()
                && let Ok(Some(_)) = conn.query_row(&sql, params![user_id], |_| Ok(())).optional()
            {
                return true;
            }
            false
        }
    }
}
