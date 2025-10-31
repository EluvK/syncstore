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
fn grant_acl_with_full_access() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc = json!({ "name": "ACL Repo", "description": "Repository for ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?.id;

    // user2 cannot access the repo
    assert_unauthorized(store.get(namespace, "repo", &repo_id, user2));

    // user1 grants user2 full access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::FullAccess);
    // only owner can create ACL
    assert_unauthorized(store.create_acl((namespace, "repo"), acl.clone(), user2));
    store.create_acl((namespace, "repo"), acl, user1)?;

    // user2 can now access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ACL Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2"));
    }
    store.update(namespace, "repo", &repo_id, &updated, user2)?;
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["description"], "Updated by user2");

    // user2 can insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &post_doc, user2)?.id;
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
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?.id;

    // user1 grants user2 read access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Read);
    store.create_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "ReadOnly Repo");

    // user2 cannot update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Attempted update by user2"));
    }
    assert_unauthorized(store.update(namespace, "repo", &repo_id, &updated, user2));

    // user2 cannot insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    assert_unauthorized(store.insert(namespace, "post", &post_doc, user2));

    // user2 cannot delete the repo
    assert_unauthorized(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

    Ok(())
}

#[test]
fn grant_edit_can_read_and_update() -> Result<(), Box<dyn std::error::Error>> {
    let s = BasicTestSuite::new()?;

    let store = s.store.clone();
    let namespace = &s.namespace;
    let user1 = &s.user1_id;
    let user2 = &s.user2_id;

    // user1 insert new repo
    let repo_doc = json!({ "name": "Edit Repo", "description": "Repository for edit ACL test", "status": "normal" });
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?.id;

    // user1 grants user2 edit access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Edit);
    store.create_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "Edit Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2 with edit access"));
    }
    store.update(namespace, "repo", &repo_id, &updated, user2)?;
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["description"], "Updated by user2 with edit access");

    // user2 cannot insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    assert_unauthorized(store.insert(namespace, "post", &post_doc, user2));

    // user2 cannot delete the repo
    assert_unauthorized(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

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
    let repo_id = store.insert(namespace, "repo", &repo_doc, user1)?.id;

    // user1 grants user2 write access to the repo
    let acl = gen_acl(&repo_id, user2, AccessLevel::Write);
    store.create_acl((namespace, "repo"), acl, user1)?;

    // user2 can access the repo
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["name"], "Write Repo");

    // user2 can update the repo
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("description".to_string(), json!("Updated by user2 with write access"));
    }
    store.update(namespace, "repo", &repo_id, &updated, user2)?;
    let item = store.get(namespace, "repo", &repo_id, user2)?;
    assert_eq!(item.body["description"], "Updated by user2 with write access");

    // user2 can insert child data (post) under the repo
    let post_doc =
        json!({ "title": "Post by user2", "category": "test", "content": "This is a test post.", "repo_id": repo_id });
    let post_id = store.insert(namespace, "post", &post_doc, user2)?.id;
    let post_item = store.get(namespace, "post", &post_id, user2)?;
    assert_eq!(post_item.body["title"], "Post by user2");

    // user2 cannot delete the repo
    assert_unauthorized(store.delete(namespace, "repo", &repo_id, user2));

    // owner user1 can still delete the repo
    store.delete(namespace, "repo", &repo_id, user1)?;
    assert_not_found(store.get(namespace, "repo", &repo_id, user1));

    Ok(())
}
