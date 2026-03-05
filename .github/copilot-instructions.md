<!-- Copilot instructions for the SyncStore Rust workspace -->
# SyncStore — AI coding quick guide

Use this guide to make safe, codebase-aligned edits quickly.

## Big picture
- Workspace has 3 crates: `syncstore` (library + HTTP router), `xss` (service binary), `ss-utils` (logging helpers).
- Main runtime path: `xss/src/main.rs` builds schemas with `collection!`, creates `Store`, then calls `syncstore::init_service`.
- `syncstore/src/lib.rs` starts two Salvo servers concurrently (`/api` and `/admin`) and wires OpenAPI/Swagger.
- Request flow: router middleware decodes JWT -> inserts `user_schema` into `Depot` -> handlers call `Store` methods.

## Core components to read first
- `syncstore/src/store.rs`: central business facade (user ops, CRUD, ACL checks, recursive parent permission logic).
- `syncstore/src/components/data_manager.rs`: namespace -> sqlite backend mapping (`:memory:` special namespace).
- `syncstore/src/components/user_manager.rs`: user/friend collections and user keypair creation.
- `syncstore/src/backend/sqlite.rs`: schema compilation, sqlite tables, ACL persistence, parent/unique handling.
- `syncstore/src/router/mod.rs` + `router/data.rs`: auth hoops, data endpoints, pagination + batch behavior.

## Project-specific patterns (important)
- No separate `AclManager` exists now; ACL logic is split between `Store` and backend ACL tables (`__acls`).
- Permission order in `Store::check_permission`: owner -> direct ACL -> recursive parent ACL (`upgrade_for_parent`).
- Collection schemas are JSON Schema draft-7 plus custom keys:
  - `x-parent-id`: enforces parent existence and drives `parent_id` relation.
  - `x-unique`: maps to sqlite `uniq` column constraint.
- Sqlite tables are sanitized/prefixed (see `sanitize_table_name`); never assume collection name == table name.
- Data list endpoints default to owner scope; `?permission=true` triggers recursive accessible-id collection.
- `router/mod.rs` injects 300ms latency (`latency_inject`) for all API requests; account for this in debugging/perf checks.

## Developer workflows
- Build all crates: `cargo build`
- Test library behavior: `cargo test -p syncstore`
- Run service locally: `cargo run -p xss -- xss/config.toml`
- Run DB migration/import tool: `cargo run -p syncstore --bin db_convert -- syncstore/src/bin/convert.toml <source.db>`

## Examples worth copying
- Schema registration macro usage: `tests/integrated/mock.rs` and `xss/src/main.rs`.
- ACL behavior expectations: `tests/integrated/acl_management.rs`.
- Error-to-HTTP status mapping: `syncstore/src/error.rs` (`ServiceError` implements `Scribe`).
- Batch payload limits and truncation markers: `syncstore/src/router/data.rs`.

## Editing guidance for agents
- Prefer extending existing builders (`DataManagerBuilder`, `SqliteBackendBuilder`) over ad-hoc initialization.
- Keep `AccessLevel` string compatibility stable; values are persisted and parsed via `FromStr` in `types.rs`.
- If changing schema/ACL internals, validate with integration tests under `syncstore/tests/integrated/`.
