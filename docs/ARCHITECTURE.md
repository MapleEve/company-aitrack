# 系统架构

## 适用对象

这篇面向希望理解 AiTrack 整体设计的开发者、运维人员和安全审查者。它描述三个组件的职责划分、数据流向、协议版本和技术选型理由。

---

## 组件概览

AiTrack 由三个独立组件构成，通过 HTTP/JSON 协议通信，所有行为准则由 `CONTRACT.md` v1.2 统一约束。

```
┌─────────────────────────────────────────────────┐
│  AI 编码工具（Claude Code / Codex CLI / Cursor）  │
│  PostToolUse / afterFileEdit / UserPromptSubmit 钩子│
└────────────────────┬────────────────────────────┘
                     │ stdin JSON
                     ▼
┌─────────────────────────────────────────────────┐
│  Rust 客户端  aitrack                            │
│  · 适配器解析  · Myers/LCS diff                  │
│  · record_sig  · SQLite 本地存储                  │
│  · flush 上传  · 节流心跳                         │
└────────────────────┬────────────────────────────┘
                     │ POST /api/v1/ai-track/edits
                     │ POST /api/v1/ai-track/heartbeat
                     ▼
┌────────────────────────────┐  ┌────────────────────────────┐
│  Java 服务端               │  │  Go 服务端                  │
│  Spring Boot 3 / H2 / PG  │  │  chi v5 / ParadeDB（生产） │
│  10 步校验链               │  │  10 步校验链（完全等价）     │
└────────────────────────────┘  └────────────────────────────┘
```

---

## 客户端：Rust CLI

**职责**：在 AI 工具的编辑事件触发时，捕获、签名、本地存储并上报一条编辑记录。

### 模块布局（六边形架构，Sprint 2）

```
src/
  main.rs / cli.rs / config.rs / lib.rs   — 命令分发、配置、入口
  git.rs / init.rs / uploader.rs / heartbeat.rs / update.rs

  domain/                   — 纯领域逻辑，无框架依赖
    mod.rs
    model.rs                — EditRecord、ApiConfig 等核心领域模型
    crypto.rs               — HMAC-SHA256、record_sig、请求签名
    diff.rs                 — Myers/LCS diff（similar crate）
    keywords.rs             — 提示词意图分类关键词（硬编码常量数组）

  port/                     — 输出端口（抽象接口）
    mod.rs
    storage.rs              — StoragePort trait（本地 SQLite 读写）
    upload.rs               — UploadPort trait（HTTP 上报）

  adapter/                  — 适配器实现
    mod.rs
    sqlite/                 — StoragePort 的 SQLite 实现（SqliteStorage）
      mod.rs / schema.rs / models.rs / queries.rs / vec.rs / keyword_store.rs
    http/                   — UploadPort 的 HTTP 实现（HttpUploader）
      mod.rs / upload.rs
    event/                  — 输入适配器（钩子事件解析）
      mod.rs / claude.rs / codex.rs / cursor.rs

  db/                       — 旧 db 模块（向后兼容保留）
  adapters/                 — 旧适配器（向后兼容保留）
  testkit/factories.rs      — 种子确定性测试工厂
```

### 三层结构（六边形架构）

Rust 客户端严格遵循六边形架构三层分离：

- **domain/**（纯领域）：`model.rs`（`EditRecord`、`ApiConfig`）、`crypto.rs`（HMAC 计算）、`diff.rs`（Myers/LCS）、`keywords.rs`（关键词常量），无任何框架依赖
- **port/**（输出端口抽象）：`StoragePort` trait（本地 SQLite 读写契约）、`UploadPort` trait（HTTP 上报契约），均为纯 Rust trait，不依赖具体实现
- **adapter/**（适配器实现）：`adapter/sqlite/`（`SqliteStorage` 实现 `StoragePort`）+ `adapter/http/`（`HttpUploader` 实现 `UploadPort`）+ `adapter/event/`（claude/codex/cursor 输入事件解析）

### HttpUploader 数据流

```
capture → lib.rs → uploader::flush_unsynced(&HttpUploader)
  → HttpUploader::post_batch → POST /api/v1/ai-track/edits/batch
  → PostBatchResult: Success / TransientError / CredentialError / UnparseableOk
  → Success        → mark_synced（synced=1，synced_at 更新）
  → TransientError → increment_retry（retry_count+1，稍后重试）
  → CredentialError→ increment_retry + 记录错误日志（401 凭证失效）
  → UnparseableOk  → mark_synced（服务端返回 2xx 但响应体无法解析，视为已接受）
```

### 本地存储

- `~/.aitrack/config.toml`（0600）：api_url、credential、device_id
- `~/.aitrack/records.db`（0600）：SQLite，存储所有捕获记录（编辑记录 + prompt_context）
- `~/.aitrack/keywords.db`（0600）：SQLite，存储关键词指纹（`keyword_fingerprint()` SHA256）

`device_id` 在首次运行时生成 UUIDv4 并持久化，后续只读。WCDB 采用多库结构：`records.db` 负责编辑记录，`keywords.db` 独立存储关键词指纹，两库职责分离。

---

## 数据流：从钩子触发到入库

### 编辑事件流（PostToolUse / afterFileEdit）

1. AI 工具触发 PostToolUse/afterFileEdit 钩子
2. aitrack capture 从 stdin 读取 JSON
3. 按 --tool 选择适配器（claude/codex/cursor）解析 payload
4. 调用 similar crate 计算 Myers/LCS diff
   → added_lines, removed_lines, diff_hunk
5. spawn git 获取 repo 元数据
   → repo_url, branch, current_sha
6. 获取 OS hostname
7. 从 prompt_context 表查询最近一次提示词 → prompt_summary（可选，仅 claude）
8. 计算 record_sig
   → HMAC_SHA256(hmac_secret, canonical_string)（不包含 prompt_summary）
9. 2 秒去重窗口检查（同 session_id + file_path）
10. INSERT INTO records（synced=0）
11. flush_unsynced → POST /api/v1/ai-track/edits
    → 服务端 10 步校验链
    → 更新 synced/retry_count

### 提示词捕获流（UserPromptSubmit，仅 Claude Code）

1. Claude Code 触发 UserPromptSubmit 钩子
2. aitrack prompt-capture 从 stdin 读取 JSON（{"session_id": "...", "prompt": "..."}）
3. 截断至 512 字符
4. INSERT INTO prompt_context（session_id, prompt_text）

---

## 服务端 10 步校验链

服务端对每批上报数据依次执行：

| 步骤 | 校验内容 | 失败结果 |
|------|----------|----------|
| 1 | Bearer token 有效 | 401 整批拒绝 |
| 2 | X-AiTrack-Timestamp 与服务器时差 ≤ 300 秒 | 401 整批拒绝 |
| 3 | X-AiTrack-Signature HMAC 验证 | 401 整批拒绝 |
| 4 | 每条 record_sig HMAC 验证 | 单条 rejected: sig_mismatch |
| 5 | diff_hunk 行数与 added/removed_lines 一致（±1） | 单条 flagged: diff_inconsistent |
| 6 | repo_url 在白名单内（enforce=true 时） | 单条 flagged/rejected: repo_unknown |
| 7 | file_path 路径合理性校验 | 单条 flagged: path_mismatch |
| 8 | added_lines ≤ max_added_lines（默认 5000） | 单条 flagged: oversized |
| 9 | 速率限制：(token, file_path) 每小时 ≤ 30 条 | 单条 rejected: rate_limited |
| 10 | accepted + flagged 记录写入数据库 | — |

flagged 记录照常入库，标记供管理员审查；rejected 记录不入库，客户端 retry_count+1。

---

## 协议 v1.2 概览

**v1.2 变更**：token 与 hmac_secret 合并为单一 credential 字符串签发；加固点编号统一为 H1–H8。

**v1.1 变更**：在编辑记录和心跳请求中新增 `hostname` 字段，记录上报机器的 OS hostname。

- `hostname` 不是访问控制机制，不做 per-token 隔离
- 同一 credential 在多台机器使用是合法场景，`hostname` 用于人工审查时区分来源

请求签名方式（两类）：

```
# 请求级签名（防重放）
X-AiTrack-Signature = HMAC_SHA256(hmac_secret, "{unix_ts}\n{sha256_hex(body_bytes)}")

# 记录级签名（防篡改/防伪造）
record_sig = HMAC_SHA256(hmac_secret, canonical_string)
```

canonical_string 字段顺序严格定义于 `CONTRACT.md`，客户端与服务端两侧必须字节一致。

---

## 技术选型理由

### 为什么用 Rust 写客户端

- 无运行时依赖，单一二进制，便于开发者安装
- 钩子命令需要低延迟（默认 10 秒超时），Rust 启动无 JVM/Node 开销
- `similar` crate 提供经过验证的 Myers/LCS diff 实现，防止行数统计被朴素算法放大

### 为什么服务端有 Java 和 Go 两套实现

两套实现在功能和协议上完全等价（wire-compatible），提供不同的运维选择：

| 维度 | Java（Spring Boot 3.3.8） | Go（chi v5.2.5 / Go 1.25）|
|------|---------------------------|---------------------------|
| 数据库 | H2（默认）/ ParadeDB（生产） | ParadeDB / PostgreSQL（生产必须）|
| 部署模型 | JRE + jar，适合现有 JVM 基础设施 | 单一二进制，distroless 镜像，适合极简容器 |
| ORM | Spring Data JPA / Hibernate | 原生 database/sql，无 ORM |
| 适用场景 | 已有 Java 技术栈的团队 | 偏好轻量容器或无 JVM 环境 |

两套实现共享同一个 e2e 测试套件（`e2e/`），以证明协议兼容性。

---

## 架构演进路线

DB-1 / DB-2 / DB-3 已全部交付。

### Phase DB-1 / DB-2 — 向量基础设施（已交付）

**状态**：客户端与两套服务端均已实现。搜索端点（Phase DB-3）同样已交付。

#### 客户端 — sqlite-vec 本地向量存储（DB-2）

客户端数据库层从单文件 `db.rs` 重构为 `db/` 模块：

| 文件 | 职责 |
|------|------|
| `mod.rs` | DB 打开、auto_extension 注册、对外 re-export |
| `schema.rs` | DDL 常量（`records`、`kv`、`vec_records`） |
| `models.rs` | `Record`、`InspectRow` 结构体 |
| `queries.rs` | 所有 CRUD 查询函数 |
| `vec.rs` | sqlite-vec 探测、`VEC_DISABLED` AtomicBool、`ensure_vec_table()` |

sqlite-vec 通过 `sqlite3_auto_extension` 在 DB 打开时自动注册。若扩展加载失败，`VEC_DISABLED` 置为 `true`，核心捕获流程不受影响（降级模式）。`vec_records` 虚拟表使用 `vec0(embedding float[384])`（384 维 MiniLM）。

`records` 表新增列：`embedding BLOB`（可空，Phase DB-3 回填）。

#### 服务端 — ParadeDB / PostgreSQL 支持（DB-1）

Java 和 Go 服务端在原有嵌入式数据库基础上，新增 PostgreSQL/ParadeDB 支持。

**Java（Spring Boot）**：通过 `SPRING_PROFILES_ACTIVE=postgres` 激活。新增五个环境变量：`AITRACK_DB_HOST`（默认 localhost）、`AITRACK_DB_PORT`（5432）、`AITRACK_DB_NAME`（aitrack）、`AITRACK_DB_USER`（aitrack）、`AITRACK_DB_PASSWORD`（aitrack_secret）。

`edit_records` 表新增列：`prompt_summary TEXT` 和 `embedding BYTEA`（均可空，Phase DB-3 回填）。

**Go（chi）**：`DATABASE_URL=postgres://user:pass@host:5432/dbname` 为**必填**环境变量，生产和 E2E 均需要真实 PostgreSQL 实例。`testapp.MemoryConfig` 仅供 Go 本地单元/集成测试在无外部依赖时构造 chi router，不用于 Docker E2E。

**ParadeDB 索引 DDL**（首次部署后执行一次）：

```sql
-- BM25 全文索引（Phase DB-3 搜索端点前置条件）
CREATE INDEX IF NOT EXISTS edits_bm25 ON edit_records
  USING bm25 (id, diff_hunk, prompt_summary) WITH (key_field = 'id');

-- HNSW 向量索引（DB-3 填充 embedding 后生效）
CREATE INDEX IF NOT EXISTS edits_hnsw ON edit_records
  USING hnsw (embedding vector_cosine_ops) WHERE embedding IS NOT NULL;
```

参考脚本：`server-java/src/main/resources/db-postgres-init.sql`。

### Phase DB-3 — 语义检索 API（已交付）

**状态**：Java 和 Go 双端均已实现。Embedding 列在回填前为空，BM25 可立即使用。

#### `GET /edits/search` — BM25 全文检索

使用 ParadeDB `|||` 算子对 `diff_hunk` 和 `prompt_summary` 做全文检索，按 `paradedb.score(id)` 降序排列。Java（`EditSearchService.searchBm25`）和 Go（`SearchHandler`）均支持 `token_key`/`repo` 可选过滤，返回 `{"query", "total", "hits"}`。

#### `POST /edits/similar` — pgvector HNSW ANN

接收 384 维查询向量，通过 `embedding <=> CAST($1 AS vector)` 余弦距离检索最近邻。仅 `embedding IS NOT NULL` 的记录参与检索，返回 `{"hits": [..., "distance": float]}`。

#### H2 / SQLite 降级

两端均在请求时检查 `isPostgres()` 标志，嵌入式数据库模式返回 HTTP 501。

#### Embedding 回填

Embedding 列不自动填充。如需启用 ANN 检索，运行回填脚本（`scripts/backfill_embeddings.py`）或由客户端 sqlite-vec 导出填充。

#### Docker Compose

`docker/docker-compose.yml` 新增 `db` 服务（`paradedb/paradedb:latest`），含 `pg_isready` 健康检查和 `pgdata` 命名卷。Java 服务容器默认使用 H2，通过 `SPRING_PROFILES_ACTIVE=postgres` 切换 PostgreSQL；Go 服务容器**必须**通过 `DATABASE_URL` 指定 PostgreSQL DSN，无内置嵌入式 DB。

---

### 数据库向量化

**客户端**：在现有 SQLite 基础上引入 `sqlite-vec` 扩展，为 `records.db` 中的编辑记录增加向量列，使本地存储具备语义相关查询能力。扩展为可选加载——不可用时自动降级为纯 SQLite 模式，不影响 `capture` 主流程。

**服务端（Java + Go 双实现）**：将服务端数据库从 PostgreSQL 迁移至 [ParadeDB](https://www.paradedb.com/)。ParadeDB 是基于 PostgreSQL 内核的扩展发行版，集成了 `pg_search`（BM25 全文检索）和 `pgvector`（向量检索）扩展，与 PostgreSQL wire protocol 完全兼容，现有 JPA / pgx 层无需重写。

**迁移一致性**：Java 和 Go 两套服务端同步迁移，共享新增向量列的 schema 定义，通过扩展后的 E2E 测试套件验证协议兼容性。

### 语义检索扩展

在数据库向量化就绪后，规划开放以下两类检索端点：

- **全文检索**：`GET /api/v1/ai-track/edits/search?q=`，基于 pg_search BM25 对编辑内容做相关性排序
- **向量 ANN 检索**：`POST /api/v1/ai-track/edits/similar`，基于 pgvector HNSW 索引检索语义相似的历史编辑记录

这两类端点与现有结构化查询端点并列存在，不破坏现有 API 兼容性，并在 `CONTRACT.md` 中增加对应 schema 定义。

### Phase 3 已交付（2026 Q4）

- **开发者使用画像 API**：`GET /api/v1/ai-track/profiles/{token_key}`，Java + Go 双端实现
- **多维画像计算**：使用频率（daily/weekly 趋势）、使用深度（行数分布、p50/p90、comment_density）、languages（按 23 种文件扩展名统计编程语言分布）、prompt_patterns（意图分类：generate/fix_debug/refactor/explain/test/other）、工具分布（claude/codex/cursor）
- **日常聚合 Job**：Java `ProfileAggregationJob`（@Scheduled 每日 02:00）；Go 同等实现
- **鉴权**：X-Admin-Key，403/404/200，不依赖 ParadeDB（H2/SQLite 均支持）

### Sprint 2 — 六边形架构重构（2026-05-20，已交付）

**状态**：三端全量完成，测试覆盖率继续 ≥ 90%。

#### Rust 客户端

重构为 `domain/`（纯领域逻辑）、`port/`（输出端口抽象）、`adapter/`（适配器实现）三层架构。详见"模块布局"章节。修复 `db/vec.rs` 中 `FLAG_MUTEX: Mutex<()>` 并发竞态。测试 291 通过。

#### Go 服务端

三层结构：

- **domain/**（`model/` + `port/`（`EditRecordPort`、`HeartbeatPort`、`TokenPort` 接口）+ `service/`（`ValidationPolicy` 值对象及各服务））
- **application/**（`IngestUsecase`、`ProfileUsecase`、`TokenUsecase`）
- **adapter/**（`db/`、`handler/`）→ **infrastructure/**（`app/`、`config/`）

测试 244 通过，95.3% 覆盖率（2026-05-20）。

#### Java 服务端

三层结构（同 Go）：

- **domain/**（`model/`（含 `PageResult<T>`，替换 Spring Page）+ `port/`（`DevicePort`、`EditRecordPort`、`TokenPort` 接口）+ `service/`）
- **application/**（各 UseCase/Service）
- **adapter/**（`db/`、`handler/`）→ **infrastructure/**（`app/`、`config/`）

`ValidationPolicy.java` 提取为纯领域值对象（移除 Spring 依赖）。`PageResult<T>` 为自定义泛型分页结果，不依赖 Spring Data Page，可跨服务端实现共享。测试 218 通过，LINE ≥ 90%（mvn verify，2026-05-20）。

#### testapp 包（Go 服务端）

`server-go/testapp/` 是专为测试设计的公开构建包，绕过 Go `internal/` 访问限制：

- `testapp.Build(cfg Config) (http.Handler, func(), error)`：创建真实 chi router 实例（含所有中间件和路由），返回清理函数
- `testapp.MemoryConfig(adminKey string) Config`：返回仅含 admin key 的测试配置，无需外部依赖，仅供本地单元/集成测试使用

该包专供本地集成测试使用，生产代码不引用；Docker E2E 使用真实 PostgreSQL 容器。

#### E2E

`e2e/` 目录包含两类测试，职责明确区分：

1. **`mock_chain_test.go`**（HTTP 形状测试）：使用 mock handler 验证请求/响应的 JSON 结构和 HTTP 状态码，无需真实 credential，Phase 4 新增 3 个场景
2. **`chain_integration_test.go`**（真实链路测试）：`httptest.NewServer(realChiRouter)` + testapp 本地路由 + 真实 HMAC 签名，验证完整 10 步校验链，包含三个场景：
   - 正常上报 → `accepted`
   - `record_sig` 被篡改 → `rejected: sig_mismatch`
   - Bearer token 缺失 → `401 未授权`

---

### Phase 4 — Prompt Capture (2026-05-19)

**Status**: Implemented in client, Java server, and Go server.

#### 提示词捕获

- 新增 `UserPromptSubmit` 钩子（仅 Claude Code）：claude Code 用户提交 prompt 时触发，aitrack 将 prompt 文本（截断至 512 字符）写入本地 `prompt_context` 表
- `records` 表新增 `prompt_summary TEXT` 列（可空），capture 流程从 `prompt_context` 查询当前 session 的最近一条提示词并附加
- `prompt_summary` 不参与 `record_sig` 计算（仅用于画像分析，不影响防篡改机制）
- 上传时 `prompt_summary` 作为可选字段随 edit 记录上报

#### 画像维度更新

| 维度 | 原实现 | 新实现 |
|------|-------|-------|
| 编程语言 | 无 | `languages`: 按文件扩展名统计（23 种语言） |
| 注释密度 | 无 | `depth.comment_density`: diff_hunk 新增行中注释行比例 |
| 提示词意图 | 无 | `prompt_patterns`: 关键词分类（generate/fix_debug/refactor/explain/test/other） |

---

## 安全设计原则

- **最小权限**：客户端配置和数据库均以 0600 权限存储
- **防篡改**：每条记录计算 record_sig，服务端重新验证
- **防重放**：请求签名包含时间戳，服务端拒绝 300 秒以外的请求
- **防伪造**：record_sig 绑定 device_id + token_key，跨设备伪造无效
- **加密存储**：hmac_secret 在服务端以 AES-256-GCM 加密存储（生产环境）

详见 [SECURITY_MODEL.md](SECURITY_MODEL.md)。
