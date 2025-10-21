use std::sync::Arc;

use serde_json::json;
use syncstore::{collection, components::DataManagerBuilder, store::Store};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = std::env::args().collect::<Vec<_>>();
    let config = config::Config::from_path(opt.get(1).unwrap_or(&"config.toml".into())).expect("Failed to load config");
    let _g = ss_utils::logs::enable_log(&config.log_config)?;

    // todo, data/user manager should either build from config, or passed in as param
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
            "x-parent-id": [
                { "field": "repo_id", "collection": "repo" }
            ],
        })
    };
    let data_manager = DataManagerBuilder::new("./db_test").add_db("xbb", xbb_schema)?.build();
    let store = Store::new(Arc::new(data_manager));
    let store = Arc::new(store);

    syncstore::init_service(store, &config.service_config).await?;
    Ok(())
}
