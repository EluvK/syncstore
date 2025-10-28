use serde_json::json;

use crate::mock::BasicTestSuite;

#[test]
fn basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;

    // insert
    let doc = json!({ "title": "Welcome", "author": "system" });
    let meta_insert = store.insert(namespace, "post", &doc, &s.user1_id)?;
    let id = meta_insert.id.clone();

    // get
    let item = store.get(namespace, "post", &id, &s.user1_id)?;
    let body = item.body;
    assert_eq!(body["title"], "Welcome");
    assert_eq!(body["author"], "system");

    // update: add a flag
    let mut updated = body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("updated".to_string(), json!(true));
    }
    let updated_meta = store.update(namespace, "post", &id, &updated, &s.user1_id)?;
    // basic sanity: updated_at should be >= created_at (depends on backend precision)
    assert!(updated_meta.updated_at >= updated_meta.created_at || updated_meta.updated_at > item.created_at);

    // get again and verify the flag
    let item = store.get(namespace, "post", &id, &s.user1_id)?;
    assert_eq!(item.body["updated"], true);

    Ok(())
}
