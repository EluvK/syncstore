use serde_json::json;

use crate::mock::*;

#[test]
fn owner_basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;

    let user = &s.user1_id;

    // insert new data
    let doc = json!({ "name": "Test Repo", "description": "A test repository", "status": "active" });
    let meta_insert = store.insert(namespace, "repo", &doc, user)?;
    let repo_id = meta_insert.id.clone();

    // get
    let item = store.get(namespace, "repo", &repo_id, user)?;
    let body = item.body;
    assert_eq!(body["name"], "Test Repo");
    assert_eq!(item.owner, *user);
    assert_eq!(item.updated_at, item.created_at);

    // update: update repo description
    let mut updated = body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated description"));
    }
    let updated_meta = store.update(namespace, "repo", &repo_id, &updated, user)?;
    assert!(updated_meta.updated_at >= updated_meta.created_at || updated_meta.updated_at > item.created_at);

    // get again and check updated description
    let item = store.get(namespace, "repo", &repo_id, user)?;
    let body = item.body;
    assert_eq!(body["description"], "Updated description");
    assert!(item.updated_at > item.created_at);

    // delete
    store.delete(namespace, "repo", &repo_id, user)?;

    // try to get deleted item
    assert_not_found(store.get(namespace, "repo", &repo_id, user));

    Ok(())
}

#[test]
fn other_access_unauthorized() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;

    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // insert new data by user1
    let doc = json!({ "name": "User1 Repo", "description": "A test repository for user1", "status": "active" });
    let meta_insert = store.insert(namespace, "repo", &doc, user1)?;
    let repo_id = meta_insert.id.clone();

    // user2 tries to get the data
    assert_unauthorized(store.get(namespace, "repo", &repo_id, user2));

    // user2 tries to update the data
    assert_unauthorized(store.update(namespace, "repo", &repo_id, &doc, user2));

    // user2 tries to delete the data
    assert_unauthorized(store.delete(namespace, "repo", &repo_id, user2));

    Ok(())
}

#[test]
fn insert_and_list_child_data() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user = &s.user1_id;

    let repo_doc = json!({ "name": "Repo for Posts", "description": "Repository to hold posts", "status": "active" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user)?.id;

    let post_doc1 = json!({ "title": "First Post", "category": "general", "content": "This is the first post.", "repo_id": repo_id });
    let post_doc2 = json!({ "title": "Second Post", "category": "general", "content": "This is the second post.", "repo_id": repo_id });

    let post_id1 = store.insert(namespace, "post", &post_doc1, user)?.id;
    let post_id2 = store.insert(namespace, "post", &post_doc2, user)?.id;

    let (posts, _next_marker) = store.list(namespace, "post", &repo_id, None, 10, user)?;
    assert_eq!(posts.len(), 2);
    let post_ids: Vec<String> = posts.into_iter().map(|p| p.id).collect();
    assert!(post_ids.contains(&post_id1));
    assert!(post_ids.contains(&post_id2));

    let user2 = &s.user2_id;
    assert_unauthorized(store.get(namespace, "post", &post_id1, user2));
    assert_unauthorized(store.list(namespace, "post", &repo_id, None, 10, user2));

    Ok(())
}
