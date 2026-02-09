use serde_json::json;
use syncstore::{collection, store::Store};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = std::env::args().collect::<Vec<_>>();
    let config = config::Config::from_path(opt.get(1).unwrap_or(&"config.toml".into())).expect("Failed to load config");
    let _g = ss_utils::logs::enable_log(&config.log_config)?;

    let xbb_schema = collection! {
        // ✅ query users' repos: list_by_owner()
        // ✅ query certain repo: get()
        "repo" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "description": { "type": ["string", "null"] },
                "status": { "type": "string", "enum": ["normal", "deleted"] }
            },
            "required": ["name", "status"]
        }),
        // ✅ query posts in certain repo: list_by_parent(repo_id)
        "post" => json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "category": { "type": "string" },
                "content": { "type": "string" },
                "repo_id": { "type": "string" }
            },
            "required": ["title", "repo_id", "category", "content"],
            "x-parent-id": { "parent": "repo", "field": "repo_id" }
        }),
        // ✅ query users' subscriptions: list_by_owner()
        // ✅ query subscribers of certain repo: list_by_parent(repo_id)
        // "subscribe" => json!({
        //     "type": "object",
        //     "properties": {
        //         "user_id": { "type": "string" },
        //         "repo_id": { "type": "string" }
        //     },
        //     "required": ["user_id", "repo_id"],
        //     "x-parent-id": { "parent": "repo", "field": "repo_id" }
        // }),
        // ✅ query comments of certain post: list_by_parent(post_id)
        "comment" => json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "post_id": { "type": "string" },
                "parent_id": { "type": ["string", "null"] },
                "paragraph_index": { "type": ["number", "null"] },
                "paragraph_hash": { "type": ["string", "null"] }
            },
            "required": ["content", "post_id"],
            "x-parent-id": { "parent": "post", "field": "post_id" }
        }),
    };
    let store = Store::build(&config.store_config.directory, vec![("xbb", xbb_schema)])?;
    syncstore::init_service(store, &config.service_config).await?;
    Ok(())
}
