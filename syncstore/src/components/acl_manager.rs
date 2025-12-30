use std::{collections::HashMap, path::Path, sync::Arc};

use crate::{
    backend::{Backend, SqliteBackend, sqlite::SqliteBackendBuilder},
    error::{StoreError, StoreResult},
    types::{AccessControl, AccessLevel, DataItem, Permission},
};

pub struct AclManager {
    backend: Arc<SqliteBackend>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PermissionSchema {
    pub data_id: String,
    pub user_id: String,
    pub access_level: AccessLevel,
}
enum InspectAclField<'a> {
    DataId(&'a str),
    UserId(&'a str),
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
                "data_id": { "type": "string" },
                "user_id": { "type": "string" },
                "access_level": {
                    "type": "string",
                    "enum": ["read", "update", "create", "write", "full_access"]
                }
            },
            "required": ["data_id", "user_id", "access_level"],
            "x-inspect": ["data_id", "user_id"]
        });

        let backend = SqliteBackendBuilder::file(path)
            .with_collection_schema(Self::ACL_TABLE, acl_schema)
            .build()?;

        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    fn list_all_acls(&self, field: InspectAclField) -> StoreResult<Vec<DataItem>> {
        let mut marker = None;
        let mut all_items = Vec::new();
        loop {
            let (items, next_marker) = match field {
                InspectAclField::DataId(data_id) => {
                    self.backend
                        .list_by_inspect(Self::ACL_TABLE, "data_id", data_id, marker, 100)?
                }
                InspectAclField::UserId(user_id) => {
                    self.backend
                        .list_by_inspect(Self::ACL_TABLE, "user_id", user_id, marker, 100)?
                }
            };
            all_items.extend(items);
            match next_marker {
                Some(m) => marker = Some(m),
                None => break,
            }
        }
        Ok(all_items)
    }

    pub fn get_user_acls(&self, user_id: &str) -> StoreResult<Vec<AccessControl>> {
        let items = self.list_all_acls(InspectAclField::UserId(user_id))?;
        let mut acl_map: HashMap<String, Vec<Permission>> = HashMap::new();
        for item in items {
            let schema = serde_json::from_value::<PermissionSchema>(item.body)?;
            let permission = Permission {
                user: schema.user_id,
                access_level: schema.access_level,
            };
            acl_map.entry(schema.data_id).or_default().push(permission);
        }
        let acls = acl_map
            .into_iter()
            .map(|(data_id, permissions)| AccessControl { data_id, permissions })
            .collect();
        Ok(acls)
    }

    pub fn get_data_acl(&self, data_id: &str) -> StoreResult<AccessControl> {
        let items = self.list_all_acls(InspectAclField::DataId(data_id))?;
        let permissions = items
            .into_iter()
            .map(|item| {
                let schema = serde_json::from_value::<PermissionSchema>(item.body)?;
                Ok::<_, StoreError>(Permission {
                    user: schema.user_id,
                    access_level: schema.access_level,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AccessControl {
            data_id: data_id.to_owned(),
            permissions,
        })
    }

    pub fn update_acl(&self, acl: AccessControl, owner: &str) -> StoreResult<()> {
        let mut new_perms_map: HashMap<String, PermissionSchema> = acl
            .permissions
            .into_iter()
            .map(|p| {
                (
                    p.user.clone(),
                    PermissionSchema {
                        data_id: acl.data_id.clone(),
                        user_id: p.user,
                        access_level: p.access_level,
                    },
                )
            })
            .collect();
        let existing_items = self.list_all_acls(InspectAclField::DataId(&acl.data_id))?;

        let mut deleted_ids = Vec::new();
        let mut to_update = Vec::new();
        for item in existing_items {
            let existing: PermissionSchema = serde_json::from_value(item.body.clone())?;
            match new_perms_map.remove(&existing.user_id) {
                Some(new_p) => to_update.push((item.id, new_p)),
                None => deleted_ids.push(item.id),
            }
        }
        for (_, p) in new_perms_map {
            self.backend
                .insert(Self::ACL_TABLE, &serde_json::to_value(p)?, owner.to_owned())?;
        }
        if !deleted_ids.is_empty() {
            self.backend.batch_delete(Self::ACL_TABLE, &deleted_ids)?;
        }
        for (id, p) in to_update {
            self.backend.update(Self::ACL_TABLE, &id, &serde_json::to_value(p)?)?;
        }

        Ok(())
    }

    pub fn delete_acls_by_data_id(&self, data_id: &str) -> StoreResult<()> {
        let ids = self
            .list_all_acls(InspectAclField::DataId(data_id))?
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            self.backend.batch_delete(Self::ACL_TABLE, &ids)?;
        }
        Ok(())
    }
}
