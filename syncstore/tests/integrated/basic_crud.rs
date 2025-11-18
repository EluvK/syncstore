use crate::mock::*;
use itertools::Itertools;
use serde_json::json;

#[test]
fn owner_basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;

    let user = &s.user1_id;

    // insert new data
    let doc = json!({ "name": "Test Repo", "description": "A test repository", "status": "normal" });
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
    let doc = json!({ "name": "User1 Repo", "description": "A test repository for user1", "status": "normal" });
    let meta_insert = store.insert(namespace, "repo", &doc, user1)?;
    let repo_id = meta_insert.id.clone();

    // user2 tries to get the data
    assert_permission_denied(store.get(namespace, "repo", &repo_id, user2));

    // user2 tries to update the data
    assert_permission_denied(store.update(namespace, "repo", &repo_id, &doc, user2));

    // user2 tries to delete the data
    assert_permission_denied(store.delete(namespace, "repo", &repo_id, user2));

    Ok(())
}

#[test]
fn insert_and_list_child_data() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user = &s.user1_id;

    let repo_doc = json!({ "name": "Repo for Posts", "description": "Repository to hold posts", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user)?.id;

    let post_doc1 = json!({ "title": "First Post", "category": "general", "content": "This is the first post.", "repo_id": repo_id });
    let post_doc2 = json!({ "title": "Second Post", "category": "general", "content": "This is the second post.", "repo_id": repo_id });

    let post_id1 = store.insert(namespace, "post", &post_doc1, user)?.id;
    let post_id2 = store.insert(namespace, "post", &post_doc2, user)?.id;

    let (posts, _next_marker) = store.list_children(namespace, "post", &repo_id, None, 10, user)?;
    assert_eq!(posts.len(), 2);
    let post_ids: Vec<String> = posts.into_iter().map(|p| p.id).collect();
    assert!(post_ids.contains(&post_id1));
    assert!(post_ids.contains(&post_id2));

    let user2 = &s.user2_id;
    assert_permission_denied(store.get(namespace, "post", &post_id1, user2));
    assert_permission_denied(store.list_children(namespace, "post", &repo_id, None, 10, user2));

    Ok(())
}

#[test]
fn insert_and_list_owner_data() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;

    let repo_doc =
        json!({ "name": "Repo for Owner Posts", "description": "Repository to hold owner posts", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?.id;

    for _ in 0..10 {
        let post_doc = json!({ "title": "Owner Post", "category": "general", "content": "This is an owner post.", "repo_id": repo_id });
        store.insert(namespace, "post", &post_doc, user1)?;
    }

    let (posts_page1, next_marker1) = store.list_by_owner(namespace, "post", None, 5, user1)?;
    assert_eq!(posts_page1.len(), 5);
    assert!(next_marker1.is_some());
    let (posts_page2, next_marker2) = store.list_by_owner(namespace, "post", next_marker1.as_deref(), 5, user1)?;
    assert_eq!(posts_page2.len(), 5);
    assert!(next_marker2.is_none());

    assert!(
        posts_page1
            .into_iter()
            .chain(posts_page2.into_iter())
            .map(|p| p.id)
            .all_unique()
    );

    Ok(())
}

#[test]
fn validate_child_parent_data() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user = &s.user1_id;

    let repo_doc = json!({ "name": "Repo for Validation", "description": "Repository to test parent-child validation", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user)?.id;

    // insert data will check permission on parent collection, so return 404 not found if parent id invalid, not even reach data schema validation
    let invalid_post_doc = json!({ "title": "Invalid Post", "category": "general", "content": "This post has an invalid repo_id.", "repo_id": "non_existent_repo_id" });
    assert_not_found(store.insert(namespace, "post", &invalid_post_doc, user));

    let valid_post_doc = json!({ "title": "Valid Post", "category": "general", "content": "This post has a valid repo_id.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &valid_post_doc, user)?.id;

    // update data will check permission but the id already exists, so passed the permission check, but failed on data schema validation
    let updated_post_doc = json!({ "title": "Updated Post", "category": "general", "content": "This post has an updated valid repo_id.", "repo_id": "non_existent_repo_id" });
    assert_validation_error(store.update(namespace, "post", &post_id, &updated_post_doc, user));

    let another_repo_doc = json!({ "name": "Another Repo", "description": "Another repository", "status": "normal" });
    let another_repo_id = store.insert(namespace, "repo", &another_repo_doc, user)?.id;
    // but you do can update to another valid parent id
    let updated_post_doc_valid = json!({ "title": "Updated Post", "category": "general", "content": "This post has an updated valid repo_id.", "repo_id": another_repo_id });
    let new_meta = store.update(namespace, "post", &post_id, &updated_post_doc_valid, user)?;
    assert!(new_meta.updated_at > new_meta.created_at);
    assert!(new_meta.id == post_id);

    Ok(())
}
