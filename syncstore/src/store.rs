use std::sync::Arc;

use serde_json::Value;

use crate::backend::Backend;
use crate::components::{DataManager, DataManagerBuilder, DataSchemas, UserManager};
use crate::error::{StoreError, StoreResult};
use crate::types::{ACLMask, AccessControl, DataItem, Id, Permission, PermissionSchema, UserSchema};

pub struct Store {
    data_manager: Arc<DataManager>,
    user_manager: Arc<UserManager>,
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

        Ok(Arc::new(Self {
            data_manager,
            user_manager,
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

    pub fn list_with_permission(
        &self,
        namespace: &str,
        collection: &str,
        marker: Option<String>,
        limit: usize,
        user: &str,
    ) -> StoreResult<(Vec<DataItem>, Option<String>)> {
        let backend = self.data_manager.backend_for(namespace)?;
        let permissions = backend.get_user_permissions(collection, user)?;
        let mut all_items = Vec::new();
        let mut next_marker = None;
        for perm in permissions {
            if let Some(marker) = &marker {
                if &perm.data_id != marker {
                    continue;
                }
            }
            let data = backend.get(collection, &perm.data_id)?;
            if all_items.len() >= limit {
                next_marker = Some(data.id.clone());
                break;
            }
            all_items.push(data);
        }
        Ok((all_items, next_marker))
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
        if let Ok(acl) = self.root_get_data_acl(namespace, collection, &data.id) {
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
    // get data acl without permission check
    fn root_get_data_acl(&self, namespace: &str, collection: &str, data_id: &str) -> StoreResult<AccessControl> {
        let backend = self.data_manager.backend_for(namespace)?;
        let permissions = backend.get_data_permissions(collection, data_id)?;
        Ok(AccessControl {
            data_id: data_id.to_string(),
            permissions: permissions
                .into_iter()
                .map(|schema| Permission {
                    user: schema.user_id,
                    access_level: schema.access_level,
                })
                .collect(),
        })
    }

    pub fn get_data_acl(
        &self,
        (namespace, collection): (&str, &str),
        data_id: &str,
        user: &str,
    ) -> StoreResult<AccessControl> {
        let data = self.get(namespace, collection, &data_id.to_string(), user)?;
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        let backend = self.data_manager.backend_for(namespace)?;
        let permissions = backend.get_data_permissions(collection, data_id)?;
        Ok(AccessControl {
            data_id: data_id.to_string(),
            permissions: permissions
                .into_iter()
                .map(|schema| Permission {
                    user: schema.user_id,
                    access_level: schema.access_level,
                })
                .collect(),
        })
    }

    /// query acls the user has access to
    pub fn get_user_acls(&self, (namespace, collection): (&str, &str), user: &str) -> StoreResult<Vec<AccessControl>> {
        let backend = self.data_manager.backend_for(namespace)?;
        let permissions = backend.get_user_permissions(collection, user)?;
        Ok(permissions
            .into_iter()
            .fold(
                std::collections::HashMap::<String, Vec<Permission>>::new(),
                |mut acc, schema| {
                    let permission = Permission {
                        user: schema.user_id.clone(),
                        access_level: schema.access_level,
                    };
                    acc.entry(schema.data_id.clone()).or_default().push(permission);
                    acc
                },
            )
            .into_iter()
            .map(|(data_id, permissions)| AccessControl { data_id, permissions })
            .collect())
    }

    pub fn update_acl(&self, (namespace, collection): (&str, &str), acl: AccessControl, user: &str) -> StoreResult<()> {
        let data = self.get(namespace, collection, &acl.data_id, user)?;
        // only owner can update ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        let backend = self.data_manager.backend_for(namespace)?;
        let new_permissions = acl
            .permissions
            .into_iter()
            .map(|perm| PermissionSchema {
                data_id: acl.data_id.clone(),
                user_id: perm.user,
                access_level: perm.access_level,
            })
            .collect::<Vec<_>>();
        backend.update_acls(collection, &data.id, &new_permissions, user)?;
        Ok(())
    }

    pub fn delete_acl(&self, (namespace, collection): (&str, &str), data_id: &str, user: &str) -> StoreResult<()> {
        let id = data_id.to_string();
        let data = self.get(namespace, collection, &id, user)?;
        // only owner can delete ACL for the data
        if data.owner != user {
            return Err(StoreError::PermissionDenied);
        }
        let backend = self.data_manager.backend_for(namespace)?;
        backend.delete_acls_by_data_id(collection, data_id)?;
        Ok(())
    }
}
