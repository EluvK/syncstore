use std::{path::Path, sync::Arc};

use crate::{
    backend::{Backend, SqliteBackend, sqlite::SqliteBackendBuilder},
    error::StoreResult,
    types::{Meta, UserSchema},
    utils::constant::{ROOT_OWNER, USER_TABLE},
};

pub struct UserManager {
    backend: Arc<SqliteBackend>,
}

impl UserManager {
    pub fn new(base_dir: impl AsRef<Path>) -> StoreResult<Self> {
        let mut path = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        path.push("users.db");

        let user_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "username": { "type": "string" },
                "password": { "type": "string" },
                "avatar_url": { "type": "string" }
            },
            "required": ["username", "password"],
            "x-unique": "username"
        });
        let backend = Arc::new(
            SqliteBackendBuilder::file(path)
                .with_collection_schema(USER_TABLE, user_schema)
                .build()?,
        );

        Ok(UserManager { backend })
    }

    pub fn create_user(&self, username: &str, password: &str) -> StoreResult<()> {
        let user = serde_json::json!({
            "username": username,
            "password": password
        });
        let meta = Meta::new(ROOT_OWNER.to_string(), Some(username.to_string()));
        self.backend.insert(USER_TABLE, &user, meta)?;
        Ok(())
    }

    pub fn validate_user(&self, username: &str, password: &str) -> StoreResult<Option<String>> {
        if let Ok(item) = self.backend.get_by_unique(USER_TABLE, username)
            && item.body.get("password") == Some(&serde_json::json!(password))
        {
            Ok(Some(item.id))
        } else {
            Ok(None)
        }
    }

    pub fn get_user(&self, user_id: &String) -> StoreResult<UserSchema> {
        let item = self.backend.get(USER_TABLE, user_id)?;
        let user_profile = serde_json::from_value::<UserSchema>(item.body)?;
        Ok(user_profile)
    }

    pub fn update_user(&self, user_id: &String, user: &UserSchema) -> StoreResult<Meta> {
        self.backend.update(USER_TABLE, user_id, &serde_json::to_value(user)?)
    }

    pub fn get_inner_backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }
}
