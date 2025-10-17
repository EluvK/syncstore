use std::sync::Arc;

use serde_json::Value;

use crate::backend::Backend;
use crate::components::{DataManager, UserManager};
use crate::error::StoreResult;
use crate::types::{DataItem, Id, Meta, Uid};

pub struct Store {
    pub data_manager: Arc<DataManager>,
    pub user_manager: Arc<UserManager>,
}

impl Store {
    pub fn new(manager: Arc<DataManager>) -> Self {
        let user_manager = Arc::new(UserManager::new("./db_test/inner").unwrap());
        Self {
            data_manager: manager,
            user_manager,
        }
    }

    /// Insert a document body. Returns meta including generated id.
    pub fn insert(
        &self,
        namespace: &str,
        collection: &str,
        body: &Value,
        owner: Uid,
        unique: Option<String>,
    ) -> StoreResult<Meta> {
        let backend = self.data_manager.backend_for(namespace)?;
        let meta = Meta::new(owner, unique);
        backend.insert(collection, body, meta)
    }

    pub fn get(&self, namespace: &str, collection: &str, id: &Id) -> StoreResult<DataItem> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.get(collection, id)
    }

    pub fn update(&self, namespace: &str, collection: &str, id: &Id, body: &Value) -> StoreResult<Meta> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.update(collection, id, body)
    }

    pub fn delete(&self, namespace: &str, collection: &str, id: &Id) -> StoreResult<()> {
        let backend = self.data_manager.backend_for(namespace)?;
        backend.delete(collection, id)
    }
}
