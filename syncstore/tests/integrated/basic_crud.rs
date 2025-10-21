use std::{fs, sync::Arc};

use serde_json::json;
use syncstore::collection;
use syncstore::components::DataManagerBuilder;
use syncstore::store::Store;
use uuid::Uuid;

#[test]
fn basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    // create a unique temporary directory so tests can run in parallel and won't pollute the repo
    let tmp = std::env::temp_dir().join(format!("syncstore_test_{}", Uuid::new_v4()));
    fs::create_dir_all(&tmp)?;

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

    let data_manager = DataManagerBuilder::new(tmp.to_str().unwrap())
        .add_db("example_ns", post_schemas)?
        .build();
    let namespace = "example_ns";
    let store = Store::new(Arc::new(data_manager));

    // insert
    let doc = json!({ "title": "Welcome", "author": "system" });
    let meta_insert = store.insert(namespace, "post", &doc, "system".to_string())?;
    let id = meta_insert.id.clone();

    // get
    let item = store.get(namespace, "post", &id)?;
    let body = item.body;
    assert_eq!(body["title"], "Welcome");
    assert_eq!(body["author"], "system");

    // update: add a flag
    let mut updated = body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("updated".to_string(), json!(true));
    }
    let updated_meta = store.update(namespace, "post", &id, &updated)?;
    // basic sanity: updated_at should be >= created_at (depends on backend precision)
    assert!(updated_meta.updated_at >= updated_meta.created_at || updated_meta.updated_at > item.created_at);

    // get again and verify the flag
    let item = store.get(namespace, "post", &id)?;
    assert_eq!(item.body["updated"], true);

    // cleanup - best effort
    let _ = fs::remove_dir_all(&tmp);

    Ok(())
}
