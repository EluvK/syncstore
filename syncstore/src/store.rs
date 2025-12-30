use std::sync::Arc;

use serde_json::Value;

use crate::backend::Backend;
use crate::components::{AclManager, DataManager, DataManagerBuilder, DataSchemas, UserManager};
use crate::error::{StoreError, StoreResult};
use crate::types::{ACLMask, AccessControl, DataItem, Id, UserSchema};

pub struct Store {
    data_manager: Arc<DataManager>,
    user_manager: Arc<UserManager>,
    acl_manager: Arc<AclManager>,
}

impl Store {
    pub fn build(base_dir: impl AsRef<std::path::Path>, dbs: Vec<(&str, DataSchemas)>) -> StoreResult<Arc<Self>> {
        let path = base_dir.as_ref().to_path_buf();
        let inner_path = path.join("inner");
        std::fs::create_dir_all(&inner_path)?;

        let mut data_manager = DataManagerBuilder::new(&path);
        for (db_name, schemas) in dbs {
            match db_name {
                "memory" => {
                    data_manager = data_manager.add_memory_db(schemas)?;
                }
                _ => {
                    data_manager = data_manager.add_db(db_name, schemas)?;
                }
            }
        }
        let data_manager = Arc::new(data_manager.build());
        let user_manager = Arc::new(UserManager::new(&inner_path)?);
        let acl_manager = Arc::new(AclManager::new(&inner_path)?);
        Ok(Arc::new(Self {
            data_manager,
            user_manager,
            acl_manager,
        }))
    }
}

/// User management operations
impl Store {
    pub fn validate_user(&self, username: &str, password: &str) -> StoreResult<Option<String>> {
        self.user_manager.validate_user(username, password)
    }
    pub fn get_user(&self, user_id: &String) -> StoreResult<UserSchema> {
        self.user_manager.get_user(user_id)
    }

    pub fn update_user(&self, user_id: &String, user_schema: &UserSchema) -> StoreResult<()> {
        self.user_manager.update_user(user_id, user_schema)
    }

    pub fn create_user(&self, username: &str, password: &str) -> StoreResult<()> {
        self.user_manager.create_user(username, password)
    }

    pub fn get_user_backend(&self) -> Arc<dyn Backend> {
        self.user_manager.get_inner_backend()
    }

    pub fn list_friends(&self, user_id: &str) -> StoreResult<Vec<(String, UserSchema)>> {
        let friend_ids = self.user_manager.list_friends(user_id)?;
        let mut friends = Vec::new();
        for friend_id in friend_ids {
            if let Ok(user_schema) = self.get_user(&friend_id) {
                friends.push((friend_id, user_schema));
            }
        }
        Ok(friends)
    }
    pub fn add_friend(&self, user_id: &String, friend_id: &String) -> StoreResult<()> {
        self.user_manager.add_friend(user_id, friend_id)?;
        self.user_manager.add_friend(friend_id, user_id)?;
        Ok(())
    }
}

/// Data operations, CRUD using data manager, re-expose here for convenience
impl Store {
    // -- CRUD operations below --
    /// Insert a document body. Returns meta including generated id.
    pub fn insert(&self, namespace: &str, collection: &str, body: &Value, user: &str) -> StoreResult<String> {
        let backend = self.data_manager.backend_for(namespace)?;
        // check permission on parent collection if exist.
        // else the collection is root level, allow insert for anyone.
        if let Some((parent_collection, field)) = backend.parent_collection(collection) {
            // get the parent field value from body
            let Some(parent_id) = body.get(field).and_then(|v| v.as_str()) else {
                return Err(StoreError::Validation(format!(
                    "missing parent id field `{}` for collection `{}`",
                    field, collection
                )));
            };
            let parent_data = backend.get(parent_collection, &parent_id.to_string())?;
            if !self.check_permission((namespace, parent_collection), &parent_data, user, ACLMask::CREATE_ONLY)? {
                return Err(StoreError::PermissionDenied);
            }
        }
        backend.insert(collection, body, user.to_string())
    }

    pub fn list_by_owner(
        &self,
        namespace: &str,
        collection: &str,
        marker: Option<String>,
        limit: usize,
        user: &str,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        // seems no need to check permission for listing by owner
        let backend = self.data_manager.backend_for(namespace)?;
        backend.list_by_owner(collection, user, marker, limit)
    }

    pub fn list_children(
        &self,
        namespace: &str,
        collection: &str,
        parent_id: &str,
        marker: Option<String>,
        limit: usize,
        user: &str,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        // list children operation should have access for the parent collection.
        let backend = self.data_manager.backend_for(namespace)?;
        let Some((parent_collection, _field)) = backend.parent_collection(collection) else {
            return Err(StoreError::NotFound(format!(
                "no parent collection for current `{}`",
                collection
            )));
        };
        let parent_data = backend.get(parent_collection, &parent_id.to_string())?;
        // check permission on parent data
        if !self.check_permission((namespace, parent_collection), &parent_data, user, ACLMask::READ_ONLY)? {
            return Err(StoreError::PermissionDenied);
        }
        backend.list_children(collection, parent_id, marker, limit)
    }

    pub fn get(&self, namespace: &str, collection: &str, id: &Id, user: &str) -> StoreResult<DataItem> {
        let backend = self.data_manager.backend_for(namespace)?;
        let data = backend.get(collection, id)?;
        // check permission
        if !self.check_permission((namespace, collection), &data, user, ACLMask::READ_ONLY)? {
            return Err(StoreError::PermissionDenied);
        }
        Ok(data)
    }

    pub fn update(
        &self,
        namespace: &str,
        collection: &str,
        id: &Id,
        body: &Value,
        user: &str,
    ) -> StoreResult<DataItem> {
        let backend = self.data_manager.backend_for(namespace)?;
        let data = backend.get(collection, id)?;
        // check permission
        if !self.check_permission((namespace, collection), &data, user, ACLMask::UPDATE_ONLY)? {
            return Err(StoreError::PermissionDenied);
        }
        backend.update(collection, id, body)
    }

    // todo delete might leave child data orphaned, need to consider how to handle it
    // add a re-mapping relation?
    pub fn delete(&self, namespace: &str, collection: &str, id: &Id, user: &str) -> StoreResult<()> {
        let backend = self.data_manager.backend_for(namespace)?;
        let data = backend.get(collection, id)?;
        // check permission
        if !self.check_permission((namespace, collection), &data, user, ACLMask::DELETE)? {
            return Err(StoreError::PermissionDenied);
        }
        backend.delete(collection, id)
    }

    /// 1. if the data owner is the user, allow
    /// 2. else check directly acl
    /// 3. else check parent data recursively
    fn check_permission(
        &self,
        (namespace, collection): (&str, &str),
        data: &DataItem,
        user: &str,
        needed_mask: ACLMask,
    ) -> StoreResult<bool> {
        // check owner
        if data.owner == user {
            return Ok(true);
        }
        // check ACL
        if let Ok(acl) = self.acl_manager.get_acl(&data.id) {
            for perm in acl.permissions {
                let acl_mask: ACLMask = perm.access_level.clone().into();
                if perm.user == user && acl_mask.contains(needed_mask) {
                    return Ok(true);
                }
            }
        }
        // check parent data recursively
        let backend = self.data_manager.backend_for(namespace)?;
        if let Some(parent_id) = data.parent_id.as_ref()
            && let Some((parent_collection, _field)) = backend.parent_collection(collection)
        {
            let parent_data = backend.get(parent_collection, parent_id)?;
            return self.check_permission((namespace, parent_collection), &parent_data, user, needed_mask);
        }
        Ok(false)
    }

    pub fn get_data_backend(&self, namespace: &str) -> StoreResult<Arc<crate::backend::SqliteBackend>> {
        self.data_manager.backend_for(namespace)
    }
}

/// ACL related operations
impl Store {
    pub fn get_acl(
        &self,
        (namespace, collection): (&str, &str),
        data_id: &str,
        user: &str,
    ) -> StoreResult<AccessControl> {
        let data = self.get(namespace, collection, &data_id.to_string(), user)?;
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        match self.acl_manager.get_acl(data_id) {
            Ok(acl) => Ok(acl),
            Err(StoreError::NotFound(_)) => {
                // return empty ACL if not found
                Ok(AccessControl {
                    data_id: data_id.to_string(),
                    permissions: Vec::new(),
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn update_acl(&self, (namespace, collection): (&str, &str), acl: AccessControl, user: &str) -> StoreResult<()> {
        let data = self.get(namespace, collection, &acl.data_id, user)?;
        // only owner can update ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        self.acl_manager.update_acl(acl, user)
    }

    pub fn delete_acl(&self, (namespace, collection): (&str, &str), data_id: &str, user: &str) -> StoreResult<()> {
        let id = data_id.to_string();
        let data = self.get(namespace, collection, &id, user)?;
        // only owner can delete ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        self.acl_manager.delete_acls_by_data_id(data_id)?;
        Ok(())
    }
}
