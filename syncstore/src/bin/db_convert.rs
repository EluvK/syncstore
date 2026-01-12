use std::path::Path;

use base64::Engine;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;
use syncstore::{
    backend::Backend,
    components::DataSchemasBuilder,
    error::StoreError,
    utils::constant::{ROOT_OWNER, USER_TABLE},
};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: db_convert <convert.toml> <source.db>");
        std::process::exit(1);
    }

    let config: MappingConfig = toml::from_str(&std::fs::read_to_string(&args[1])?)?;

    println!("Loaded mapping config: {:#?}", config);

    let source_db = &args[2];

    if !Path::new(source_db).exists() {
        eprintln!("Source database file does not exist: {}", source_db);
        std::process::exit(1);
    }

    let conn = Connection::open_with_flags(source_db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)?;

    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")?;
    let table_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;

    // debug print all tables and their columns
    for table in table_iter {
        let table = table?;
        println!("\nProcessing table: {}", table);

        let sel = conn.prepare(&format!("SELECT * FROM {}", table))?;
        let col_names = sel
            .column_names()
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        // let mut rows = sel.query([])?;
        println!("Found columns: {:?}", col_names);
    }

    // init target store
    let mut schemas = DataSchemasBuilder::new();
    for (collection, schema_str) in config
        .data_mappings
        .iter()
        .map(|m| (&m.target_collection, &m.target_schema))
    {
        let schema_json: serde_json::Value = serde_json::from_str(schema_str)?;
        schemas = schemas.add_schema(collection, schema_json);
    }
    let schemas = schemas.build();

    let store = syncstore::store::Store::build(
        &config.general.target_db_path,
        vec![(&config.general.namespace, schemas)],
    )?;

    // user import
    if let Some(user_table) = config.user_mapping.map(|u| u.source_table) {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {}", user_table))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get("id")?;
            let username: String = row.get("username").or(row.get("name"))?;
            let password: String = row.get("password")?;
            let created_at = row.get("created_at")?;
            let updated_at = row.get("updated_at")?;

            let (pk, sk) = syncstore::utils::hpke::generate_keypair();
            let body = json!({
                "username": username,
                "password": password,
                "public_key": base64::engine::general_purpose::STANDARD.encode(&pk),
                "secret_key": base64::engine::general_purpose::STANDARD.encode(&sk),
            });

            println!("Imported user: {}", &id);

            let user_backend = store.get_user_backend();
            match user_backend.import(USER_TABLE, &body, ROOT_OWNER.to_string(), id, created_at, updated_at) {
                Ok(_id) => (),
                Err(ref e @ StoreError::Validation(ref err)) => {
                    if err.clone().to_ascii_lowercase().contains("unique constraint failed") {
                        println!(" [SKIP] User {} already exists, skipping.", username);
                    } else {
                        return Err(anyhow::anyhow!("Failed to insert user {}: {}", username, e));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to insert user {}: {}", username, e));
                }
            }
        }
    };

    // data import
    let now = chrono::Utc::now();
    for mapping in config.data_mappings.iter() {
        println!(
            "--------------\nImporting data from table: {} to collection: {}",
            &mapping.source_table, &mapping.target_collection
        );
        let mut stmt = conn.prepare(&format!("SELECT * FROM {}", mapping.source_table))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = if let Some(id_field) = &mapping.id_field {
                row.get(id_field.as_str())?
            } else {
                row.get("id").or(row.get("_id")).unwrap_or_else(|_| {
                    println!(
                        "[WARN] ID field not specified for table {}, generating new ID.",
                        mapping.source_table
                    );
                    uuid::Uuid::new_v4().to_string()
                })
            };
            let created_at = if let Some(created_at_field) = &mapping.created_at_field {
                row.get(created_at_field.as_str())?
            } else {
                row.get("created_at").unwrap_or_else(|_| {
                    println!(
                        "[WARN] Created at field not specified for table {}, using current time.",
                        mapping.source_table
                    );
                    now
                })
            };
            let updated_at = if let Some(updated_at_field) = &mapping.updated_at_field {
                row.get(updated_at_field.as_str())?
            } else {
                row.get("updated_at").unwrap_or_else(|_| {
                    println!(
                        "[WARN] Updated at field not specified for table {}, using current time.",
                        mapping.source_table
                    );
                    now
                })
            };
            let owner: String = row.get(mapping.owner_field.as_str())?;

            let mut body_map = std::collections::HashMap::new();
            for field in &mapping.data_fields {
                let value: Option<String> = row.get(field.as_str())?;
                if let Some(value) = value {
                    body_map.insert(field.clone(), value);
                }
            }
            let body = serde_json::to_value(body_map)?;

            println!(
                "Imported data item: {} into collection: {}",
                &id, &mapping.target_collection
            );

            let data_backend = store.get_data_backend(&config.general.namespace)?;
            match data_backend.import(
                &mapping.target_collection,
                &body,
                owner.clone(),
                id.clone(),
                created_at,
                updated_at,
            ) {
                Ok(_) => (),
                Err(ref e @ StoreError::Validation(ref err)) => {
                    if err.clone().to_ascii_lowercase().contains("unique constraint failed") {
                        println!(
                            " [SKIP] Data item {} in collection {} already exists, skipping.",
                            id, &mapping.target_collection
                        );
                    } else {
                        return Err(anyhow::anyhow!(
                            "Failed to insert data item {} into collection {}: {}",
                            id,
                            &mapping.target_collection,
                            e
                        ));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to insert data item {} into collection {}: {}",
                        id,
                        &mapping.target_collection,
                        e
                    ));
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct MappingConfig {
    general: GeneralConfig,
    user_mapping: Option<UserMapping>,
    data_mappings: Vec<DataMapping>,
}

#[derive(Debug, Deserialize)]
struct GeneralConfig {
    target_db_path: String,
    namespace: String,
}

#[derive(Debug, Deserialize)]
struct UserMapping {
    source_table: String,
}

#[derive(Debug, Deserialize)]
struct DataMapping {
    // default value id / _id
    id_field: Option<String>,
    // default value created_at or take now time
    created_at_field: Option<String>,
    // default value updated_at or take now time
    updated_at_field: Option<String>,

    source_table: String,
    target_collection: String,
    target_schema: String,

    owner_field: String,
    data_fields: Vec<String>,
}
