use std::{path::PathBuf, sync::Arc};

use serde_json::json;
use syncstore::{collection, store::Store};

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
        println!("created temp dir: {}", tmp.path().display());

        let post_schemas = collection! {
            "post" => json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "author": { "type": "string" }
                },
                "required": ["title", "author"],
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
