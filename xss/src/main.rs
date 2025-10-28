use serde_json::json;
use syncstore::{collection, store::Store};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = std::env::args().collect::<Vec<_>>();
    let config = config::Config::from_path(opt.get(1).unwrap_or(&"config.toml".into())).expect("Failed to load config");
    let _g = ss_utils::logs::enable_log(&config.log_config)?;

    let xbb_schema = collection! {
        "repo" => json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "description": { "type": "string" },
                "status": { "type": "string" }
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
    let store = Store::build("./db_test", vec![("xbb", xbb_schema)])?;
    syncstore::init_service(store, &config.service_config).await?;
    Ok(())
}
