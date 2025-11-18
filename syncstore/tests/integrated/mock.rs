use std::{path::PathBuf, sync::Arc};

use serde_json::json;
use syncstore::{
    collection,
    error::{StoreError, StoreResult},
    store::Store,
};

pub fn assert_not_found<T: std::fmt::Debug>(result: StoreResult<T>) {
    match result {
        Err(StoreError::NotFound(_)) => {}
        _rest => panic!("Expected NotFound error, got: {:?}", _rest),
    }
}

pub fn assert_permission_denied<T: std::fmt::Debug>(result: StoreResult<T>) {
    match result {
        Err(StoreError::PermissionDenied) => {}
        _rest => panic!("Expected PermissionDenied error, got: {:?}", _rest),
    }
}

pub fn assert_validation_error<T: std::fmt::Debug>(result: StoreResult<T>) {
    match result {
        Err(StoreError::Validation(_)) => {}
        _rest => panic!("Expected ValidationError error, got: {:?}", _rest),
    }
}

/// Test suite to setup and teardown test environment
///
/// usage:
/// ```
/// let s = BasicTestSuite::new().unwrap();
/// ```
pub struct BasicTestSuite {
    // even hold the temp dir to keep it alive during the test
    // still result the tmp file exist after the test, do not know why.
    // manually try clean at drop results in a OS file busy error on Windows.
    _tmp: tempfile::TempDir,
    pub path: PathBuf,
    pub store: Arc<Store>,
    pub namespace: String,
    pub user1_id: String,
    pub user2_id: String,
}

impl BasicTestSuite {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        let path = tmp.path().to_path_buf();
        // println!("created temp dir: {}", tmp.path().display());

        let post_schemas = collection! {
            "repo" => json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "status": { "type": "string", "enum": ["normal", "deleted"] }
                },
                "required": ["name", "status"],
                "x-unique": "name",
            }),
            "post" => json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "category": { "type": "string" },
                    "content": { "type": "string" },
                    "repo_id": { "type": "string" }
                },
                "required": ["title", "repo_id"],
                "x-parent-id": { "parent": "repo", "field": "repo_id" },
            })
        };
        let namespace = "example_ns".to_string();
        let store = Store::build(&tmp, vec![(&namespace, post_schemas)])?;

        store.create_user("user1", "p1")?;
        store.create_user("user2", "p2")?;

        let user1_id = store.validate_user("user1", "p1")?.unwrap();
        let user2_id = store.validate_user("user2", "p2")?.unwrap();

        Ok(Self {
            _tmp: tmp,
            path,
            store,
            namespace,
            user1_id,
            user2_id,
        })
    }
}
