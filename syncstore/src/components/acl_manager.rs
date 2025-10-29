use std::{path::Path, sync::Arc};

use crate::{
    backend::{Backend, SqliteBackend, sqlite::SqliteBackendBuilder},
    error::StoreResult,
    types::{AccessControl, Meta},
};

pub struct AclManager {
    backend: Arc<SqliteBackend>,
}

impl AclManager {
    const ACL_TABLE: &str = "acls";

    pub fn new(base_dir: impl AsRef<Path>) -> StoreResult<Self> {
        let mut path = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        path.push("acls.db");

        let acl_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "data_id": {
                    "type": "string"
                },
                "permissions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "user": {
                                "type": "string"
                            },
                            "access_level": {
                                "type": "string",
                                "enum": ["read", "edit", "write", "full_access"]
                            }
                        }
                    }
                }
            },
            "x-unique": "data_id"
        });

        let backend = SqliteBackendBuilder::file(path)
            .with_collection_schema(Self::ACL_TABLE, acl_schema)
            .build()?;

        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    pub fn create_acl(&self, acl: AccessControl, user: &str) -> StoreResult<()> {
        let meta = Meta::new(user.to_string(), Some(acl.data_id.clone()));
        self.backend
            .insert(Self::ACL_TABLE, &serde_json::to_value(acl)?, meta)?;
        Ok(())
    }

    pub fn get_acl(&self, data_id: &str) -> StoreResult<AccessControl> {
        let acl_item = self.backend.get_by_unique(Self::ACL_TABLE, data_id)?;
        let acl: AccessControl = serde_json::from_value(acl_item.body)?;
        Ok(acl)
    }

    pub fn update_acl(&self, acl: AccessControl) -> StoreResult<()> {
        let existing_acl = self.backend.get_by_unique(Self::ACL_TABLE, &acl.data_id)?;
        self.backend
            .update(Self::ACL_TABLE, &existing_acl.id, &serde_json::to_value(acl)?)?;
        Ok(())
    }

    pub fn delete_acl_by_data_id(&self, data_id: &str) -> StoreResult<()> {
        let existing_acl = self.backend.get_by_unique(Self::ACL_TABLE, data_id)?;
        self.backend.delete(Self::ACL_TABLE, &existing_acl.id)?;
        Ok(())
    }
}
