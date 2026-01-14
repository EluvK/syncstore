use std::{path::Path, sync::Arc};

use base64::Engine;

use crate::{
    backend::{Backend, SqliteBackend, sqlite::SqliteBackendBuilder},
    error::StoreResult,
    types::{UserSchema, UserSchemaDocument},
    utils::constant::{FRIENDS_TABLE, ROOT_OWNER, USER_TABLE},
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
                "avatar_url": { "type": "string" },
                "public_key": { "type": "string", "contentEncoding": "base64" },
                "secret_key": { "type": "string", "contentEncoding": "base64" }
            },
            "required": ["username", "password", "public_key", "secret_key"],
            "x-unique": "username"
        });
        let friend_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "friend_id": { "type": "string" },
                "unique_key": { "type": "string" },
            },
            "required": ["friend_id"],
            "x-parent-id": { "parent": USER_TABLE, "field": "friend_id" },
            "x-unique": "unique_key"
        });
        let backend = Arc::new(
            SqliteBackendBuilder::file(path)
                .with_collection_schema(USER_TABLE, user_schema)
                .with_collection_schema(FRIENDS_TABLE, friend_schema)
                .build()?,
        );

        Ok(UserManager { backend })
    }

    pub fn create_user(&self, username: &str, password: &str) -> StoreResult<()> {
        let (sk, pk) = crate::utils::hpke::generate_keypair();
        let user = serde_json::json!({
            "username": username,
            "password": password,
            "public_key": base64::engine::general_purpose::STANDARD.encode(&pk),
            "secret_key": base64::engine::general_purpose::STANDARD.encode(&sk),
        });
        self.backend.insert(USER_TABLE, &user, ROOT_OWNER.to_string())?;
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
        let user_profile = serde_json::from_value::<UserSchemaDocument>(item.body)?;
        Ok(UserSchema::from_document(user_id.clone(), user_profile))
    }

    pub fn update_user(&self, user_id: &String, user: &UserSchema) -> StoreResult<()> {
        self.backend.update(
            USER_TABLE,
            user_id,
            &serde_json::to_value(UserSchemaDocument::from(user.clone()))?,
        )?;
        Ok(())
    }

    pub fn get_inner_backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }

    pub fn add_friend(&self, user_id: &String, friend_id: &String) -> StoreResult<()> {
        let body = serde_json::json!({
            "friend_id": friend_id,
            "unique_key": format!("{}:{}", user_id, friend_id),
        });
        self.backend.insert(FRIENDS_TABLE, &body, user_id.to_string())?;
        Ok(())
    }

    pub fn list_friends(&self, user_id: &str) -> StoreResult<Vec<String>> {
        // todo better with pagination
        let items = self.backend.list_by_owner(FRIENDS_TABLE, user_id, None, 100)?;
        let friend_ids = items
            .0
            .into_iter()
            .filter_map(|item| {
                item.body
                    .get("friend_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        Ok(friend_ids)
    }
}
