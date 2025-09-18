# SyncStore 设计与开发指南（Rust 实现）

本项目目标：实现一个支持多 namespace、多 collection、基于 JSON Schema 校验的数据同步存储模块，内置用户管理与权限校验。

---

## 核心设计

### 1. Namespace

* 每个 namespace 对应一个独立的 SQLite 数据文件。
* 用于隔离不同业务或多租户场景。

### 2. Collection

* 每个 namespace 下包含若干 collection。
* 每个 collection 由 **JSON Schema** 定义，规定数据结构、字段类型和引用关系。
* collection 数据存储在 SQLite 的一张表里。

### 3. Schema Registry

* 在 namespace 内维护已注册的 collection schema。
* Schema 一旦注册，不允许变更（只允许新增 collection），保证历史数据一致性。

### 4. 元数据管理

所有 collection 记录默认附带元数据字段：

* `id`: 主键（UUID / ULID）。
* `created_at`, `updated_at`: 时间戳。
* `owner`: 引用 `user.id`，表明数据所属用户。
* `references`: 记录当前数据引用的其他 collection 记录（跨 collection 引用

这些字段由 syncstore 自动维护，用户无需手动填充。

### 5. 用户管理（内置）

* 用户数据作为一个 **特殊的内置 collection (`user`)**，而不是依赖外部后端。
* 典型 schema:

```jsonc
{
  "title": "User",
  "type": "object",
  "properties": {
    "id": { "type": "string" },
    "name": { "type": "string" },
    "role": { "type": "string", "enum": ["admin", "member"] },
    "avatar_url": { "type": "string", "format": "uri" }
  },
  "required": ["id", "name", "role"]
}
```

* 权限校验在 syncstore 内完成：

  * 根据 `role` 和 `owner` 判断是否允许写操作。
  * 其他 collection 可通过 `$ref: "user.id"` 引用用户。

这样实现后，user 既保持 schema 驱动的一致性，又能承担权限校验的核心职责。

### 6. 数据引用

* collection schema 内可定义引用字段，例如：

```jsonc
{
  "title": "Post",
  "type": "object",
  "properties": {
    "id": { "type": "string" },
    "title": { "type": "string" },
    "author": {
      "type": "string",
      "$ref": "user.id"
    }
  },
  "required": ["id", "title", "author"]
}
```

* 在写入时，syncstore 会校验：

  * `author` 必须存在于 `user` collection。
  * 其他跨 collection 引用也做同样的校验。

### 7. 同步与变更通知

* 每条记录的 `updated_at` 用于版本控制。
* 客户端可通过 summary（变更摘要）先拉取更新，再按需请求具体数据。
* summary 不包含正文，只标记变更集合，减少流量。

---

## 开发注意事项

1. **Rust 库**

   * JSON Schema 校验：推荐 [`jsonschema`](https://crates.io/crates/jsonschema)。
   * SQLite 访问：使用 `rusqlite` 或 `sqlx`。

2. **一致性保障**

   * Schema 注册后锁定，避免后续破坏历史数据。
   * 引用校验必须在写入时进行，避免“悬挂引用”。

3. **可扩展性**

   * user 虽然是内置的，但依然是 schema 驱动的 collection。
   * 将来可扩展更多内置 collection（如 system log）。
