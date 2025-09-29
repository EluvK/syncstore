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
    const USER_TABLE: &str = "users";

    pub fn new(base_dir: impl AsRef<Path>) -> StoreResult<Self> {
        let mut path = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        path.push("users.db");

        let user_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "username": { "type": "string" },
                "password": { "type": "string" }
            },
            "required": ["username", "password"],
            "x-unique": "username"
        });
        let backend = Arc::new(
            SqliteBackendBuilder::file(path)
                .with_collection_schema(UserManager::USER_TABLE, user_schema)
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
        let meta = Meta::new("root".to_string(), Some(username.to_string()));
        self.backend.insert(UserManager::USER_TABLE, &user, meta)?;
        Ok(())
    }

    pub fn validate_user(&self, username: &str, password: &str) -> StoreResult<Option<String>> {
        if let Ok((value, meta)) = self.backend.get_by_unique(UserManager::USER_TABLE, username)
            && value.get("password") == Some(&serde_json::json!(password))
        {
            Ok(Some(meta.id))
        } else {
            Ok(None)
        }
    }

    pub fn get_user(&self, user_id: &String) -> StoreResult<String> {
        let (_value, meta) = self.backend.get(UserManager::USER_TABLE, user_id)?;
        Ok(meta.id)
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

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_user_creation() {
        let user_manager = UserManager::new("./test_db").unwrap();
        let res = user_manager.create_user("test_user", "test_password");
        println!("User creation result: {:?}", res);
        // assert!(res.is_ok());
        let v1 = user_manager.validate_user("test_user", "test_password").unwrap();
        assert!(v1.is_some());
        println!("Validation with correct password: {:?}", v1);
        let v2 = user_manager.validate_user("test_user", "wrong_password").unwrap();
        assert!(v2.is_none());
        let v3 = user_manager.validate_user("nonexistent", "test_password").unwrap();
        assert!(v3.is_none());
    }
}
