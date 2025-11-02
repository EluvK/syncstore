<!-- Copilot instructions for the SyncStore Rust workspace -->
# SyncStore — guidance for AI code agents

This file contains concise, actionable information to help AI coding agents be immediately productive when editing or extending the SyncStore codebase.

Key goals for agents:
- Understand the high-level architecture and where business logic lives.
- Follow project-specific patterns for data modelling, backends, ACLs, and JWT auth.
- Use the repo's builder patterns and prefer existing helper macros and types.

High-level architecture (quick):
- Crate root: `syncstore/` — library crate with core abstractions and a small HTTP service initializer in `src/lib.rs`.
- Major components:
  - `src/store.rs` — Store facade that wires together `DataManager`, `UserManager`, and `AclManager`. This is the main entrypoint for CRUD and ACL checks.
  - `src/components/` — contains `data_manager.rs`, `user_manager.rs`, `acl_manager.rs` and builders. Use `DataManagerBuilder` and `DataSchemasBuilder` to register DBs and collection schemas.
  - `src/backend/sqlite.rs` — Sqlite-backed implementation of `Backend`. Important responsibilities: schema registration (`__schemas`), JSON Schema validation (including custom `x-parent-id` check), table creation, unique field handling, and parent-child relations.
  - `src/router/` — HTTP routing (Salvo). Authentication, JWT handling, and endpoint handlers live here. `create_router` wires routes and JWT hoops.
  - `src/utils/jwt.rs` — central JWT helpers; `init_service` in `src/lib.rs` calls `set_jwt_config` at startup.

Important design notes (do not change lightly):
- Data schemas are stored per-collection (JSON Schema) and compiled into jsonschema validators in `sqlite.rs`. Any change to schema storage or validation must preserve compatibility with `init_collection_schema` and the `x-parent-id` custom keyword.
- Namespaces map to separate sqlite databases or an in-memory namespace `":memory:"` (see `components::MEMORY_NAMESPACE`). Use `DataManagerBuilder` to create backends.
- Table names are sanitized with `sanitize_table_name` and prefixed with `c_`. Do not assume raw collection names match table names.
- ACL checks are performed at multiple layers: Store first checks owner, then ACL manager, then parent chain recursively (see `Store::check_permission`). Respect this order when adding new permission checks.

Developer workflows and commands (copyable):
- Run example/demo:
  - From repository root: `cargo run --example demo` (see `README.md` in crate root)
- Run the crate tests: `cargo test -p syncstore`
- Build the workspace (top-level Cargo.toml manages workspace deps): `cargo build` or `cargo build -p syncstore`

Patterns and conventions specific to this repo:
- Builder patterns: prefer `XBuilder` (e.g., `SqliteBackendBuilder`, `DataManagerBuilder`) to construct components and register schemas.
- Schemas: collection schemas are JSON Schema (draft-7) values stored and compiled in `sqlite.rs`. Custom keywords such as `x-parent-id` and `x-unique` are used; register these when adding new schema-related logic.
- Ownership and meta: `Meta` contains `id`, `owner`, `unique` and `parent_id`. Many operations rely on `Meta` fields — keep Meta creation/unwrapping consistent (see `Store::insert`).
- Error handling: project uses `thiserror` and custom `StoreError`/`ServiceError` types. Return `StoreResult<T>` or `ServiceResult<T>` as appropriate.
- JWT: `utils/jwt.rs` stores secrets in `OnceLock` and provides `generate_jwt_token`, `generate_refresh_token`, and `verify_refresh_token`. `create_router` uses `JwtAuth` with `ConstDecoder` and finds tokens in header or `jwt_token` query param.

Integration points and external dependencies:
- Salvo (HTTP framework + OpenAPI/Swagger wiring) — routes and middleware are in `src/router/` and `src/lib.rs` (OpenAPI is assembled in `init_service`).
- sqlite via `r2d2_sqlite` and `rusqlite` — concurrency uses `r2d2` pools.
- jsonschema crate with custom keyword support — used heavily in `src/backend/sqlite.rs`.
- `jsonwebtoken` for JWT operations.

Examples from the codebase to reference when making edits:
- Use `collection! { "posts" => json!(...) }` macro in `components::data_manager.rs` as an example of schema registration.
- See `Store::insert` / `Store::get` for how permission checks and parent traversal are performed.
- See `sqlite.rs` `init_collection_schema` for how the system compiles and caches validators and creates sanitized tables.
- See `router/mod.rs` and `utils/jwt.rs` for how JWT auth is injected and converted to a `user_id` in the request `Depot`.

When editing code, prefer these low-risk practices:
- Add unit tests under `tests/integrated/` for any API or storage behavior change. There are integration tests that exercise ACL and CRUD flows.
- If changing schema storage or validation, provide migration steps and ensure `__schemas` table access remains consistent.
- Keep public function signatures stable; prefer adding new functions rather than changing existing contracts unless you also update all call sites.

Files and directories to inspect first for context:
- `src/store.rs` — overall Store facade and permission logic
- `src/components/` — DataManager/ACL/User manager implementations
- `src/backend/sqlite.rs` — core storage, schema, and validation logic
- `src/router/` — routing, JWT hooks, request->user translation
- `src/utils/jwt.rs` — JWT helpers and secrets wiring
- `tests/integrated/` — examples of expected behavior (ACL, CRUD, user mgmt)

If something is missing or unclear:
- Ask for the target: lib change, new API, migration, or bugfix. Point to a file and a failing test or an example. Include the rust toolchain version if environment-specific.

End of file.
