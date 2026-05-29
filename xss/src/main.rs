use serde_json::json;
use syncstore::{collection, store::Store};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let config_path = args.get(1).map_or("config.toml", String::as_str);
    let config = config::Config::from_path(config_path).expect("Failed to load config");

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
    let tracker_schema = collection! {
        "tracker" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "description": { "type": ["string", "null"] },
                "category": { "type": "string" },
                "type": { "type": "string" },
                "config": {
                    "oneOf": [
                        { "type": "object", "properties": { "period_days": { "type": "integer" } }, "required": ["period_days"] },
                        { "type": "object", "properties": { "goal_type": { "type": "string" }, "target_value": { "type": "string" }, "progress_mode": { "type": "string", "enum": ["accumulate", "latest"] } }, "required": ["goal_type", "target_value"] },
                        { "type": "object", "properties": { "base_date": { "type": "string", "format": "date-time" }, "is_lunar": { "type": "boolean" }, "remind_type": { "type": "string" } }, "required": ["base_date", "is_lunar", "remind_type"] }
                    ]
                }
            },
            "required": ["name", "category", "type", "config"]
        }),
        "record" => json!({
            "type": "object",
            "properties": {
                "tracker_id": { "type": "string" },
                "timestamp": { "type": "string", "format": "date-time" },
                "value": { "type": ["string", "null"] },
                "content": { "type": ["string", "null"] }
            },
            "required": ["tracker_id", "timestamp"],
            "x-parent-id": { "parent": "tracker", "field": "tracker_id" }
        }),
    };
    let task_schema = collection! {
        "check_list" => json!({
            "type": "object",
            "properties": {
                "tasks": { "type": "string" },
                "archived": { "type": "boolean" },
                "archived_at": { "type": ["string", "null"] }
            },
            "required": ["tasks", "archived"]
        }),
    };
    let clipboard_history_schema = collection! {
        "entry" => json!({
            "type": "object",
            "properties": {
                "data": { "type": "string" },
            },
            "required": ["data"]
        }),
    };

    let store = Store::build(
        &config.store_config.directory,
        vec![
            ("xbb", xbb_schema),
            ("tracker", tracker_schema),
            ("task", task_schema),
            ("clipboard_history", clipboard_history_schema),
        ],
    )?;
    syncstore::init_service(store, &config.service_config).await?;
    Ok(())
}
