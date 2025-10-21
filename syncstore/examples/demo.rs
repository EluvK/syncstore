use std::sync::Arc;

use serde_json::json;
use syncstore::collection;
use syncstore::components::DataManagerBuilder;
use syncstore::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // open an in-memory sqlite backend for the example

    let post_schemas = collection! {
        "post" => json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "author": { "type": "string", "db_exists": {"collection": "users", "column": "id"} }
            },
            "required": ["title", "author"],
            "x-unique": "title",
            // "additionalProperties": false
        })
    };

    let data_manager = DataManagerBuilder::new("./db_test")
        // .add_memory_db()?
        .add_db("example_ns", post_schemas)?
        .build();
    let namespace = "example_ns";
    let store = Store::new(Arc::new(data_manager));

    // insert
    let doc = json!({ "title": "Welcome", "author": "system" });
    let meta = store.insert(namespace, "post", &doc, "system".to_string())?;
    println!("Inserted id: {}", meta.id);

    // get
    let item = store.get(namespace, "post", &meta.id)?;
    println!("Got doc: {}", item.body);

    // update: add a flag
    let mut updated = item.body.clone();
    if let serde_json::Value::Object(ref mut map) = updated {
        map.insert("updated".to_string(), json!(true));
    }
    let updated_meta = store.update(namespace, "post", &meta.id, &updated)?;
    println!("Updated at: {}", updated_meta.updated_at);

    // get again
    let item = store.get(namespace, "post", &meta.id)?;
    println!("After update: {}", item.body);

    // delete
    // store.delete(namespace, "post", &meta.id)?;
    // println!("Deleted id: {}", meta.id);

    // // try get -> expect error
    // match store.get(namespace, "post", &meta.id) {
    //     Ok(_) => println!("Unexpected: document still exists"),
    //     Err(e) => println!("Get after delete: error: {}", e),
    // }

    Ok(())
}
