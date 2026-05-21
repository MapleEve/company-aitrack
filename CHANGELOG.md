# Changelog

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format.

---

## [CI] — 2026-05-21

### Added

- `.github/workflows/fossa.yml`: FOSSA license compliance scan, runs on push to `main` and weekly schedule
- `.github/workflows/stale.yml`: disabled placeholder for stale issue/PR management (no-op until configured)

---

## [v1.6.1] — 2026-05-21

### Changed

- **Go 服务端迁移为 PostgreSQL-only**：移除 `modernc.org/sqlite` 依赖，生产与 E2E 均需提供 `DATABASE_URL`；`testapp.MemoryConfig` 仅保留用于本地单元测试构造 chi router，不再用于 Docker E2E
- `docker/docker-compose.yml`：Go 服务容器改为 `DATABASE_URL` 连接 `db`（ParadeDB）服务，移除 `aitrack-go-data` SQLite 卷

### Fixed

- E2E：`e2e/run.sh` Go 路径在 `pg_isready` 通过后新增 `sleep 2`，消除首次镜像拉取场景下 PostgreSQL 内部初始化竞态；新增容器日志抓取，方便排查 Go 服务端启动失败原因
- Go 服务端：测试占位符统一改为 `$N`（兼容 pgx/PostgreSQL）；覆盖率命令新增 `-coverpkg=./internal/...` 确保跨包覆盖统计准确；`page` 参数加 `clamp ≥ 0` 防止 `OFFSET` 为负

### 覆盖率

| 组件 | 测试数 | 行覆盖率 |
|------|--------|---------|
| Go 服务端 | 244 | 95.3% |

---

## [v1.6.0] — 2026-05-20

### 发布说明

v1.6.0（Sprint 2）完成三端完整六边形架构重构，新增 `aitrack update` ed25519 更新命令，实现真实 HTTP 上报（非 stub），并引入基于 in-memory SQLite 的 E2E 真实链路测试。

### 新增

- **`aitrack update` 子命令**：ed25519 签名验证（硬编码公钥）；从 GitHub Releases API 拉取最新版本，下载二进制 + `.sig`，验签后原子替换当前可执行文件；全零占位公钥触发启动断言拒绝
- **关键词库防篡改检测**（Keyword tamper detection）：关键词以编译期常量硬编码；`keyword_fingerprint()` 计算 SHA256 并存入 `~/.aitrack/keywords.db`（WCDB 多库：`records.db` + `keywords.db`）；指纹不匹配时告警，二进制副本为权威来源
- **`server-go/testapp` 包**：导出 `Build()` + `MemoryConfig(adminKey)`，绕过 Go `internal` 访问限制；E2E 和集成测试可无外部进程启动真实 chi router + in-memory SQLite
- **真实链路 E2E 集成测试**（`e2e/chain_integration_test.go`）：`httptest.NewServer` 接入真实 Go chi router + in-memory SQLite；3 场景：完整 happy path（accepted=3）、篡改 `record_sig` → rejected（`sig_mismatch`）、无 Bearer token → 401
- **`domain/model/PageResult<T>`**（Java）：框架无关 `record PageResult<T>(List<T> content, long totalElements)`，替代 `EditRecordPort` 中的 Spring `Page<T>`；domain 层零 Spring 导入

### 变更

- **六边形架构三端全量落地（Sprint 2）**
  - Rust 客户端：删除遗留 `db/`、`adapters/`、`crypto.rs`、`diff.rs` shim 层（共 1 927 行）；`lib.rs` 全部通过 `StoragePort`（SqliteStorage）和 `UploadPort`（HttpUploader）路由；`uploader::flush_unsynced` 接收 `&HttpUploader` 并委托 HTTP POST 至 `HttpUploader::post_batch`
  - Go 服务端：`StatsRow` 从 `domain/port` 迁移至 `domain/model`；`IngestUsecase.saveEdit` 现在返回并传播 `error`（原来静默丢弃）；三个适配器均添加编译期接口断言（`var _ port.X = (*Y)(nil)`）
  - Java 服务端：`EditRecordPort` 使用 `PageResult<T>`（无 `org.springframework.data.domain` 导入）；字段 `editRecordRepository` 在 IngestService / StatsService / ValidationService 中统一重命名为 `editRecordPort`
- **`HttpUploader::upload_batch` 真实实现**：从 `Ok(())` stub 升级为完整 HTTP POST 实现；`build_payload` 将 `Record` 切片映射为 wire JSON；`post_batch` 返回 `PostBatchResult` 枚举：`Success` / `TransientError` / `CredentialError` / `UnparseableOk`；含 13 个 wiremock 单元测试

### 覆盖率

| 组件 | 测试数 | 行覆盖率 |
|------|--------|---------|
| Rust 客户端 | 233 | 90.71% |
| Java 服务端 | 218 | LINE ≥ 90% |
| Go 服务端 | — | 95.3% |

---

## [v1.5.0] — 2026-05-20

### 发布说明

v1.5.0 完成 Phase 4 提示词捕获流水线：新增 `UserPromptSubmit` 钩子捕获用户提示词，`prompt_summary` 随编辑记录上报，服务端画像新增 `prompt_patterns` 意图分类维度。

### 新增

- **Phase 4：提示词捕获流水线**（Prompt Capture Pipeline）
  - 客户端：与 `PostToolUse` 并行安装 `UserPromptSubmit` 钩子（仅限 Claude Code）；新增 `prompt-capture` 子命令，将用户提示词（≤512 字符）存入本地 `prompt_context` SQLite 表
  - 客户端：`capture` 流程将最近一条 session 提示词作为可选 `prompt_summary` 附加到编辑记录
  - 数据库：新增 `prompt_context` 表（session_id, prompt_text, created_at）；`records` 表通过迁移新增 `prompt_summary TEXT` 列
  - 画像 API：`prompt_patterns` 维度 — 基于 `prompt_summary` 文本的关键词意图分类（generate / fix_debug / refactor / explain / test / other）
  - 画像维度重设计：`scenarios` → `languages`（基于文件扩展名，23 种类型）+ `depth.comment_density`（diff_hunk 新增行中注释行比例）
  - `CONTRACT.md` 更新：`prompt-capture` 命令、`UserPromptSubmit` 钩子模板、可选 `prompt_summary` 字段、`prompt_patterns` / `languages` / `comment_density` 画像 schema

### 覆盖率

| 组件 | 测试数 |
|------|--------|
| Rust 客户端 | 200 |
| Java 服务端 | 215 |
| Go 服务端 | 全量通过 |

---

## [v1.4.0] — 2026-05-19

### 发布说明

v1.4.0 完成 Phase 3 开发者 AI 工具使用画像：按需三维聚合（频率 / 深度 / 场景）+ 每日定时预热任务，Java 和 Go 双端功能完全对等。

### 新增

- **Phase 3：开发者 AI 工具使用画像**（Developer AI Usage Profiles）
  - Java `ProfileController`：`GET /api/v1/ai-track/profiles/{token_key}`，X-Admin-Key 鉴权
  - Java `ProfileService`：按需三维画像（使用频率 / 深度 / 场景 / 工具类型），`classifyScenario()` 路径启发式分类
  - Java `ProfileAggregationJob`：`@Scheduled(cron="0 0 2 * * *")` 每日凌晨预热
  - Go `ProfileHandler`：与 Java 功能完全对等，JSON schema 相同
  - 新增 `EditRecordRepository.findByTokenKeyAndStatusNot()` 和 `TokenRepository.findByTokenKeyAndActiveTrue()`
  - `AiTrackServerApplication` 添加 `@EnableScheduling`
  - `CONTRACT.md` §5 更新：Phase 3 画像端点完整 schema

### 文档

- `docs/PRIVACY.md`（两仓库同步）：数据采集透明度说明
- `CONTRACT.md` §5：Phase 3 画像端点 schema

### 覆盖率

| 组件 | 测试数 | 覆盖率 |
|------|--------|--------|
| Java 服务端 | 206 | — |
| Go 服务端 | — | 92.4% |

---

## v1.3.0 — 2026-05-19

### 发布说明

v1.3.0 完成三个 DB 阶段：DB-1 接入 ParadeDB/PostgreSQL 服务端、DB-2 客户端 sqlite-vec 向量扩展、DB-3 语义搜索 API。

### 新增

**Phase DB-1 — ParadeDB / PostgreSQL 服务端支持**
- Java 服务端：`postgres` Spring Profile，通过 `SPRING_PROFILES_ACTIVE=postgres` 激活
- Go 服务端：`DATABASE_URL` 环境变量切换至 PostgreSQL；未设置时回退到嵌入式 SQLite
- `edit_records` 表：新增可空列 `embedding BYTEA/BLOB` 和 `prompt_summary TEXT`
- docker-compose：新增 `paradedb/paradedb:latest` 服务，含 `pg_isready` 健康检查

**Phase DB-2 — 客户端 sqlite-vec 向量扩展**
- 重构 `client/src/db.rs` → `client/src/db/` 模块（mod / schema / models / queries / vec）
- sqlite-vec 通过 `sqlite3_auto_extension` 注册；`VEC_DISABLED` 标志用于优雅降级
- `records` 表：新增可空列 `embedding BLOB`
- 新增 `vec_records` 虚拟表（`vec0(embedding float[384])`，384 维 MiniLM 空间）

**Phase DB-3 — 语义搜索 API**
- `GET /api/v1/ai-track/edits/search?q=`：ParadeDB BM25 全文检索（`|||` 运算符）
- `POST /api/v1/ai-track/edits/similar`：pgvector HNSW ANN 近似相似度（384 维余弦距离）
- 两个端点均支持可选 `token_key`/`repo` 过滤；H2/SQLite 模式下返回 HTTP 501
- Java `EditSearchController` + `EditSearchService`；Go `SearchHandler` + `SimilarHandler`
- `CONTRACT.md` 新增两个端点的完整请求/响应 schema

### 工具链

- Go 1.24 → **1.25**（pgx v5.9.x 要求）
- JaCoCo **0.8.11 → 0.8.13**（Java 25 字节码支持）
- `pgx/v5` **5.7.2 → 5.9.2**（修复 1 个 Critical + 1 个 Low CVE）
- `golang.org/x/crypto` 升级（修复 1 个 High + 2 个 Medium CVE）

### 覆盖率

| 组件 | 测试数 | 行覆盖率 |
|------|--------|---------|
| Rust 客户端 | 196 | 91.79% |
| Java 服务端 | 186 | 95% |
| Go 服务端 | 70 | 93.2% |

---

## v1.2.0 — 2026-05-18

### 发布说明

v1.2.0 是协议 v1.2 对应的正式版本。核心变更是将 `token` 与 `hmac_secret` 合并为单个 **credential** 字符串（`<token>-<hmac_secret>`），简化了签发与分发流程。同步完成了一批安全加固，覆盖服务端请求体限制、批量上限、HMAC 常量时间比对、H2 控制台禁用，以及运行时版本升级。

### 新增

- **协议 v1.2 合并凭据（credential）**：`POST /admin/tokens` 响应字段由 `token` + `hmac_secret` 合并为单一 `credential` 字段（格式：`<token>-<hmac_secret>`）；客户端 `config.toml` 存储键由 `token`/`hmac_secret` 改为 `credential`；CLI 参数 `--credential` 接收合并字符串。
- 客户端 `init.rs`：`config.toml` 和 `records.db` 改为原子创建，先写临时文件再原子 rename，避免写入中断留下损坏文件。
- 客户端 `capture`：stdin 读取增加上限（防止超大 payload 阻塞进程）。

### 变更

- `CONTRACT.md` 升版至 v1.2，新增 `v1.2 change` 说明段落及 `Credential` 章节，明确 credential 拆分规则（按第一个 `-` 拆分）。
- Java 服务端升级至 Spring Boot **3.3.8**。
- Go 服务端依赖升级：chi **v5.2.5**；Go 工具链要求 **1.24**。
- 服务端请求体上限统一设为 **8 MiB**（Java `spring.servlet.multipart.max-request-size` / Go 中间件 `http.MaxBytesReader`）。
- 服务端单次上报 `edits` 数组上限设为 **500 条**，超出返回 400。
- 服务端 HMAC 比对全部改为**常量时间比较**（Java `MessageDigest.isEqual`，Go `subtle.ConstantTimeCompare`），消除 timing attack 面。
- Java 服务端 H2 Web 控制台在生产 Profile 下**强制禁用**（`spring.h2.console.enabled=false`）。

### 加固点

本版本加固点覆盖 H1–H8（含本版新增的服务端加固项）：

| 编号 | 说明 |
|------|------|
| H1 | record_sig HMAC — 防本地 DB 篡改 |
| H2 | record_sig 绑定 device_id+token — 防跨设备伪造 |
| H3 | 心跳 hook 状态上报 — 检测静默卸载 |
| H4 | Myers/LCS 真差分 — 防行数膨胀 |
| H5 | 速率限制 (token, file_path) 每小时 ≤ 30 — 防刷量 |
| H6 | 适配器解析失败记录日志 — 不静默吞错 |
| H7 | repo_url 白名单（enforce=true 时强制拒绝）— 防 repo 伪造 |
| H8 | file_path 合理性校验（无 `..`）— 防路径注入 |

---

## v1.1.0 — 2026-05-17

### 发布说明

v1.1.0 是协议 v1.1 对应的正式版本，核心变更是在上报记录和心跳中引入 `hostname` 字段，使同一 token 在多台机器上的编辑活动可被逐机器追溯。

### 新增

- `hostname` 字段加入 `records` 表 schema 及上报 JSON 结构（`CONTRACT.md` §Upload Request）。
- `record_sig` HMAC 计算绑定 `hostname`，防止跨设备伪造（hardening point H1/H2）。
- 心跳请求中携带 `hostname`，服务端 `devices` 接口可见每台机器的心跳状态。
- Rust 客户端 `capture` 流程第 6 步：从 OS 读取 hostname 并写入本地 SQLite 和上报体。
- Java 服务端 `ValidationService` 新增 hostname 存储与设备去重逻辑。
- Go 服务端同步支持 hostname 字段解析和存储。

### 变更

- `CONTRACT.md` 升版至 v1.1，新增 `v1.1 change` 说明段落，字段顺序文档更新。
- 服务端 `record_sig` 校验规范字符串加入 `hostname` 行（`CONTRACT.md` §Record Signature）。
- `HmacSecretEncryptorTest`、`SignatureServiceCanonicalTest` 同步更新预期值。

### 技术说明

`hostname` 是透明可见性机制，不作为访问控制手段；同一 token 多机使用属正常场景，管理员可通过 `/api/v1/ai-track/devices` 逐机审查。

---

## v1.0.0 — 2026-05-01

### 发布说明

v1.0.0 是 aitrack 的初始正式版本，建立了 Rust 客户端 + Java 服务端的双组件架构，以及完整的加固校验链（hardening points H1–H6）。

### 新增

- Rust CLI 客户端，支持 `init / remove / capture / inspect / stats / status / clean / heartbeat` 全套命令。
- 支持 Claude Code、Codex CLI、Cursor 三种 AI 编码工具的 hook 安装与卸载，操作幂等。
- 本地 SQLite 存储（`~/.aitrack/records.db`，权限 0600），`config.toml`（权限 0600）持久化配置与 `device_id`。
- Myers/LCS 真差分算法（`similar` crate），防止朴素行数统计被刷高（hardening point H4）。
- `record_sig` HMAC-SHA256 签名，绑定 `token_key + device_id + timestamp + tool + file_path + repo_url + current_sha + added_lines + removed_lines + sha256(diff_hunk)`，防止本地记录篡改（hardening point H1/H2）。
- 请求级 HMAC 签名（`X-AiTrack-Signature`），防止重放攻击，时间窗口 300 秒（hardening point H2）。
- 心跳机制：每次 `capture` 结束后节流发送，1 小时内最多一次；`aitrack heartbeat` 可强制发送（hardening point H3）。
- 适配器解析失败写本地日志，不静默吞错（hardening point H6）。
- Java 服务端（Spring Boot 3 / JDK 17 / H2 或 PostgreSQL），10 步校验链，覆盖签名、重放、差分一致性、仓库白名单、路径合理性、行数上限、速率限制。
- `AES-256-GCM` 加密存储 `hmac_secret`（`HmacSecretEncryptor`）。
- 服务端 testkit 工厂模式（`EditDtoFactory`、`TamperedFactory` 等），JaCoCo 覆盖率门槛 ≥ 90%。
- Rust 客户端测试覆盖率：行 87.75%，函数 90.24%，含 `testkit/factories.rs` 种子确定性构建器。
- `CONTRACT.md` 作为客户端与服务端的协议单一可信来源（Single Source of Truth）。

### 技术说明

初始版本以单机自托管为主要场景；H2 内存/文件数据库开箱即用，生产环境可切换 PostgreSQL（`application.yml` 配置切换，无需修改业务代码）。
