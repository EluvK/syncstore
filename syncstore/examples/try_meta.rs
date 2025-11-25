use jsonschema::paths::{LazyLocation, Location};
use jsonschema::{Keyword, ValidationError, ValidationOptions};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::Arc;

/// 数据库模拟器
#[derive(Clone)]
struct DbChecker {
    users: Arc<HashSet<String>>,
}

impl DbChecker {
    fn check(&self, value: &str) -> bool {
        self.users.contains(value)
    }
}

/// 自定义关键字实现
struct DbExists {
    table: String,
    column: String,
    checker: DbChecker,
}

impl Keyword for DbExists {
    fn validate<'i>(&self, instance: &'i Value, location: &LazyLocation) -> Result<(), ValidationError<'i>> {
        let location: Location = (&location.clone()).into();
        let val = match instance.as_str() {
            Some(v) => v,
            None => {
                return Err(ValidationError::custom(
                    location.clone(),
                    location.clone(),
                    instance,
                    "db_exists: expected string",
                ));
            }
        };
        if !self.checker.check(val) {
            return Err(ValidationError::custom(
                location.clone(),
                location,
                instance,
                format!("db_exists: value '{}' not found in {}.{}", val, self.table, self.column),
            ));
        }
        Ok(())
    }

    fn is_valid(&self, instance: &Value) -> bool {
        instance.as_str().is_some_and(|v| self.checker.check(v))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {"type": "string"},
            "name": {"type": ["string", "null"]},
        },
        "required": ["id"]
    });

    let data1 = serde_json::from_str("{\"id\": \"123\", \"name\": \"Alice\"}")?;
    let data2 = serde_json::from_str("{\"id\": \"456\"}")?;
    let data3 = serde_json::from_str("{\"id\": \"789\", \"name\": null}")?;

    let options = jsonschema::draft7::options();
    let compiled = options.build(&schema)?;
    match compiled.validate(&data1) {
        Ok(_) => println!("data1 is valid"),
        Err(e) => println!("data1 is invalid: {:?}", e),
    }
    match compiled.validate(&data2) {
        Ok(_) => println!("data2 is valid"),
        Err(e) => println!("data2 is invalid: {:?}", e),
    }
    match compiled.validate(&data3) {
        Ok(_) => println!("data3 is valid"),
        Err(e) => println!("data3 is invalid: {:?}", e),
    }

    Ok(())
}

fn _main() -> Result<(), Box<dyn std::error::Error>> {
    // 模拟数据库
    let mut set = HashSet::new();
    set.insert("u1".to_string());
    set.insert("u2".to_string());

    let checker = DbChecker { users: Arc::new(set) };

    // schema 里写我们自定义的关键字 db_exists
    let schema = json!({
        "type": "object",
        "properties": {
            "user_id": {
                "type": "string",
                "db_exists": { "table": "users", "column": "id" }
            }
        },
        "required": ["user_id"]
    });

    // 待验证的对象
    let instance_ok = json!({ "user_id": "u1" });
    let instance_bad = json!({ "user_id": "u999" });

    // 注册关键字
    let mut options = ValidationOptions::default();
    options = options.with_keyword("db_exists", {
        let checker = checker.clone();
        move |_parent: &Map<String, Value>, value: &Value, _location| {
            let table = value["table"].as_str().unwrap_or("").to_string();
            let column = value["column"].as_str().unwrap_or("").to_string();

            Ok(Box::new(DbExists {
                table,
                column,
                checker: checker.clone(),
            }))
        }
    });

    let compiled = options.build(&schema)?;

    println!("Validating instance_ok: {:?}", instance_ok);
    match compiled.validate(&instance_ok) {
        Ok(_) => println!("✅ ok"),
        Err(e) => {
            println!("❌ errors: {:?}", e);
        }
    }

    println!("Validating instance_bad: {:?}", instance_bad);
    match compiled.validate(&instance_bad) {
        Ok(_) => println!("✅ bad instance passed?"),
        Err(e) => {
            println!("❌ {:?}", e);
        }
    }

    Ok(())
}
