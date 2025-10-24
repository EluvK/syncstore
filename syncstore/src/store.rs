use std::sync::Arc;

use serde_json::Value;

use crate::backend::Backend;
use crate::components::{AclManager, DataManager, UserManager};
use crate::error::{StoreError, StoreResult};
use crate::types::{AccessControl, DataItem, Id, Meta};

pub struct Store {
    pub data_manager: Arc<DataManager>,
    pub user_manager: Arc<UserManager>,
    pub acl_manager: Arc<AclManager>,
}

impl Store {
    pub fn new(manager: Arc<DataManager>) -> Self {
        let user_manager = Arc::new(UserManager::new("./db_test/inner").unwrap());
        let acl_manager = Arc::new(AclManager::new("./db_test/inner").unwrap());
        Self {
            data_manager: manager,
            user_manager,
            acl_manager,
        }
    }
}

/// Data operations, CRUD using data manager, re-expose here for convenience
impl Store {
    /// Insert a document body. Returns meta including generated id.
    pub fn insert(&self, namespace: &str, collection: &str, body: &Value, user: &str) -> StoreResult<Meta> {
        let backend = self.data_manager.backend_for(namespace)?;
        let meta = Meta::new(user.to_string(), None);
        backend.insert(collection, body, meta)
    }

    pub fn list(
        &self,
        namespace: &str,
        collection: &str,
        parent_id: Option<&str>,
        limit: usize,
        marker: Option<&str>,
        user: &str,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.list(collection, parent_id, limit, marker, user)
    }

    pub fn get(&self, namespace: &str, collection: &str, id: &Id, user: &str) -> StoreResult<DataItem> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.get(collection, id, user)
    }

    pub fn update(&self, namespace: &str, collection: &str, id: &Id, body: &Value, user: &str) -> StoreResult<Meta> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.update(collection, id, body, user)
    }

    pub fn delete(&self, namespace: &str, collection: &str, id: &Id, user: &str) -> StoreResult<()> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.delete(collection, id, user)
    }
}

/// ACL related operations
impl Store {
    pub fn create_acl(&self, (namespace, collection): (&str, &str), acl: AccessControl, user: &str) -> StoreResult<()> {
        let data = self.get(namespace, collection, &acl.data_id, user)?;
        // only owner can set ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        self.acl_manager.create_acl(acl, user)?;
        Ok(())
    }

    // (namespace, collection): (&str, &str),
    pub fn get_acl(&self, data_id: &str, user: &str) -> StoreResult<AccessControl> {
        let acl = self.acl_manager.get_acl(data_id, user)?;
        Ok(acl)
    }

    pub fn update_acl(&self, (namespace, collection): (&str, &str), acl: AccessControl, user: &str) -> StoreResult<()> {
        let data = self.get(namespace, collection, &acl.data_id, user)?;
        // only owner can update ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        self.acl_manager.update_acl(acl, user)?;
        Ok(())
    }

    pub fn delete_acl(&self, (namespace, collection): (&str, &str), data_id: &str, user: &str) -> StoreResult<()> {
        let id = data_id.to_string();
        let data = self.get(namespace, collection, &id, user)?;
        // only owner can delete ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        self.acl_manager.delete_acl_by_data_id(data_id, user)?;
        Ok(())
    }
}
