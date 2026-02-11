use serde_json::json;
use syncstore::types::{AccessControl, AccessLevel, Permission};

use crate::mock::*;

fn gen_acl(data_id: &str, user: &str, access_level: AccessLevel) -> AccessControl {
    AccessControl {
        data_id: data_id.to_string(),
        permissions: vec![Permission {
            user: user.to_string(),
            access_level,
        }],
    }
}

#[test]
fn acl_basic_crud() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc =
        json!({ "name": "ACL CRUD Repo", "description": "Repository for ACL CRUD test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user1 creates ACL for user2
    let acl = gen_acl(&repo_id, user2, AccessLevel::Write);
    store.update_acl((namespace, "repo"), acl.clone(), user1)?;

    // user2 can update the repo with ACL
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ACL CRUD Repo");
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2 with ACL"));
    }
    let item = store.update(namespace, "repo", &repo_id, &updated, user2)?;
    assert_eq!(item.body["description"], "Updated by user2 with ACL");

    // user1 gets the ACL
    let fetched_acl = store.get_data_acl((namespace, "repo"), &repo_id, user1)?;
    assert_eq!(fetched_acl.data_id, repo_id);
    assert_eq!(fetched_acl.permissions.len(), 1);
    assert_eq!(fetched_acl.permissions[0].user, *user2);
    assert_eq!(fetched_acl.permissions[0].access_level, AccessLevel::Write);

    let user_acls = store.get_user_acls((namespace, "repo"), user1)?;
    assert_eq!(user_acls.len(), 0);
    let user_acls = store.get_user_acls((namespace, "repo"), user2)?;
    assert_eq!(user_acls.len(), 1);
    assert_eq!(user_acls[0].data_id, repo_id);

    // user1 updates the ACL to give user2 only read access
    let updated_acl = gen_acl(&repo_id, user2, AccessLevel::Read);
    store.update_acl((namespace, "repo"), updated_acl.clone(), user1)?;

    // user2 can still get the repo, but cannot update now
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ACL CRUD Repo");
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Attempted update by user2 "));
    }
    assert_permission_denied(store.update(namespace, "repo", &repo_id, &updated, user2));

    // user2 can not delete the ACL
    assert_permission_denied(store.delete_acl((namespace, "repo"), &repo_id, user2));

    // user1 deletes the ACL
    store.delete_acl((namespace, "repo"), &repo_id, user1)?;

    Ok(())
}

#[test]
fn grant_acl_with_full_access() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc = json!({ "name": "ACL Repo", "description": "Repository for ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user2 cannot access the repo
    assert_permission_denied(store.get(namespace, "repo", &repo_id, user2));

    // user1 grants user2 full access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::FullAccess);
    // only owner can create ACL
    assert_permission_denied(store.update_acl((namespace, "repo"), acl.clone(), user2));
    store.update_acl((namespace, "repo"), acl, user1)?;

    // user2 can now access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ACL Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2"));
    }
    let item = store.update(namespace, "repo", &repo_id, &updated, user2)?;
    assert_eq!(item.body["description"], "Updated by user2");

    // user2 can insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &post_doc, user2)?;
    let post_item = store.get(namespace, "post", &post_id, user2)?;
    assert_eq!(post_item.body["title"], "Post by user2");

    // user2 can even delete the repo
    store.delete(namespace, "repo", &repo_id, user2)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user2));

    Ok(())
}

#[test]
fn grant_read_can_only_get() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc =
        json!({ "name": "ReadOnly Repo", "description": "Repository for read-only ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user1 grants user2 read access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Read);
    store.update_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ReadOnly Repo");

    // user2 cannot update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Attempted update by user2"));
    }
    assert_permission_denied(store.update(namespace, "repo", &repo_id, &updated, user2));

    // user2 cannot insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    assert_permission_denied(store.insert(namespace, "post", &post_doc, user2));

    // user2 cannot delete the repo
    assert_permission_denied(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

    Ok(())
}

#[test]
fn grant_update_can_read_and_update() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc =
        json!({ "name": "Update Repo", "description": "Repository for update ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user1 grants user2 update access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Update);
    store.update_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "Update Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2 with update access"));
    }
    let item = store.update(namespace, "repo", &repo_id, &updated, user2)?;
    assert_eq!(item.body["description"], "Updated by user2 with update access");

    // user2 cannot insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    assert_permission_denied(store.insert(namespace, "post", &post_doc, user2));

    // user2 cannot delete the repo
    assert_permission_denied(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

    Ok(())
}

#[test]
fn grant_append_can_read_and_create() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc =
        json!({ "name": "Create Repo", "description": "Repository for create ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user1 grants user2 append access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::ReadAppend1);
    store.update_acl((namespace, "repo"), acl, user1)?;

    // user1 put a post under the repo to test parent permission check
    let post_doc = json!({ "title": "Initial Post", "category": "test", "content": "This is the initial post.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &post_doc, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "Create Repo");

    // user2 cannot update the repo or post in the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Attempted update by user2"));
    }
    assert_permission_denied(store.update(namespace, "repo", &repo_id, &updated, user2));
    // try to update the post
    let post_item = store.get(namespace, "post", &post_id, user1)?;
    let mut post_updated = post_item.body.clone();
    if let serde_json::Value::Object(ref mut map) = post_updated {
        map.insert("content".to_string(), json!("Attempted update of post by user2"));
    }
    assert_permission_denied(store.update(namespace, "post", &post_id, &post_updated, user2));

    // user2 can add child data (post) under the repo
    let new_post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    let new_post_id = store.insert(namespace, "post", &new_post_doc, user2)?;
    let new_post_item = store.get(namespace, "post", &new_post_id, user2)?;
    assert_eq!(new_post_item.body["title"], "Post by user2");
    assert_eq!(new_post_item.owner, *user2);

    // user2 can add comment under the post
    let comment_doc = json!({ "content": "This is a comment by user2.", "post_id": post_id });
    let comment_id = store.insert(namespace, "comment", &comment_doc, user2)?;
    let comment_item = store.get(namespace, "comment", &comment_id, user2)?;
    assert_eq!(comment_item.body["content"], "This is a comment by user2.");
    assert_eq!(comment_item.owner, *user2);

    // user2 cannot delete the repo
    assert_permission_denied(store.delete(namespace, "repo", &repo_id, user2));

    Ok(())
}

#[test]
fn grant_write_can_read_update_insert() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc = json!({ "name": "Write Repo", "description": "Repository for write ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?;

    // user1 grants user2 write access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Write);
    store.update_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "Write Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2 with write access"));
    }
    let item = store.update(namespace, "repo", &repo_id, &updated, user2)?;
    assert_eq!(item.body["description"], "Updated by user2 with write access");

    // user2 can insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &post_doc, user2)?;
    let post_item = store.get(namespace, "post", &post_id, user2)?;
    assert_eq!(post_item.body["title"], "Post by user2");

    // user2 cannot delete the repo
    assert_permission_denied(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

    Ok(())
}
