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

    const ROOT_OWNER: &str = "root";

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
        let meta = Meta::new(UserManager::ROOT_OWNER.to_string(), Some(username.to_string()));
        self.backend.insert(UserManager::USER_TABLE, &user, meta)?;
        Ok(())
    }

    pub fn validate_user(&self, username: &str, password: &str) -> StoreResult<Option<String>> {
        if let Ok(item) = self.backend.get_by_unique(UserManager::USER_TABLE, username)
            && item.body.get("password") == Some(&serde_json::json!(password))
        {
            Ok(Some(item.id))
        } else {
            Ok(None)
        }
    }

    pub fn get_user(&self, user_id: &String) -> StoreResult<String> {
        let item = self.backend.get(UserManager::USER_TABLE, user_id)?;
        Ok(item.id)
    }
}
