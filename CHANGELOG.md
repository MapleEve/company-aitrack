# Changelog

所有重要变更按 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) 格式记录。

---

## [v1.8.0] — 2026-07-03

### 发布摘要

v1.8.0 将默认本地扫描矩阵扩展到 35 个规范工具 key，并把各工具的本地来源按字段级原生读取、本地派生读取和辅助状态/用量来源合并为 agent 级完整数据面。本版本同时补齐批次上限、本地 outbox 保留策略、服务端聚合 upsert 和原文清洗策略，避免默认扫描、上传或服务端落库在大数据目录下无界增长。

GitHub Release 正文见 [`docs/RELEASE_NOTES_v1.8.0.md`](docs/RELEASE_NOTES_v1.8.0.md)。

### 新增

- **35 个默认扫描工具 key**：默认扫描覆盖 `claude`、`codex`、`cursor`、`trae`、`qwen`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode`。
- **agent 级完整数据面**：默认 key 都必须通过本地来源组合覆盖提示词、助手输出、工具调用、工具结果、用量、会话、时间和模型字段；字段缺失不补假数据。
- **用量依据区分**：用量上报保留 `usage_basis=native` / `usage_basis=local_derived`，管理员可以区分来源计量和本地派生计量。
- **服务端原文清洗**：Java 与 Go 服务端都会在保留标量、签名和时间信息的前提下清洗旧的 `diff_hunk`、`metadata` 和 `prompt_summary` 原文字段。

### 变更

- **上传批次有界**：编辑事件客户端按 200 条和 7 MiB 拆包；usage rollup 单 payload 最多 500 items；本地 usage outbox 限制行数、payload 总字节、重试次数和 pending TTL。
- **服务端聚合落库**：Java 与 Go 服务端对 usage rollup 按 `(token_key, device_id, day, agent, model, account, usage_basis)` 幂等 upsert，不保存提示词或工具结果原文。
- **默认扫描性能保护**：保留 30 天窗口、单次扫描文件数、候选数、目录遍历项、JSON/CSV/SQLite 行数、zstd 解压大小和 sidecar fan-out 上限，并通过文件缓存跳过未变化来源。
- **使用文档**：README、API、隐私、安全、部署、开发和工具支持文档同步到 v1.8.0 支持范围。

### 测试 / CI

- 客户端 E2E 矩阵门禁：35 个默认规范 key 覆盖率必须为 100%。
- 本地来源矩阵自检：67 个 source entry。
- PR CI 门禁覆盖 Rust / Java / Go 构建与覆盖率、架构门禁、Java + Go E2E、Rust 客户端本地来源 E2E、Codecov、FOSSA 和自动 review 检查。

## [v1.7.0] — 2026-06-18

### 发布摘要

v1.7.0 将 aitrack 从三条固定原生钩子扩展为「原生编辑证据 + 动态状态心跳 + 本地用量扫描」三层采集模型。本版本保持 `EditRecord` 监控事件与标量用量汇总分离，新增 Java 与 Go 两端的用量 API，并让 Rust 客户端从本机会话目录、JSON/JSONL/NDJSON、CSV、SQLite 和本地客户端状态中采集工具用量，无需用户手动粘贴第三方服务 token。

GitHub Release 正文见 [`docs/RELEASE_NOTES_v1.7.0.md`](docs/RELEASE_NOTES_v1.7.0.md)。

### 新增

- **动态工具注册表 / 状态 / 心跳**：客户端状态和心跳 payload 现在按工具 key 表达动态注册项，不再固定为三工具布尔图。
- **扩展本地来源矩阵**：默认本地扫描覆盖 30 个已验证本地来源的工具 key；显式 `--tool` 还接受 `roocode`、`kilo-code`、`gajae-code` 作为别名。没有真实来源证据的登记项不计入默认采集能力。
- **用量数据面**：客户端新增 `usage_sessions`、`usage_daily_model_rollups`、`usage_subscription_snapshots`、`usage_outbox` 表；Java 与 Go 服务端新增 `/api/v1/ai-track/usage/rollup`、`/api/v1/ai-track/usage/subscription`、`/api/v1/ai-track/usage/summary`。
- **本地会话监控恢复**：本地会话记录可为没有原生编辑钩子的工具恢复有界提示词、工具调用、窗口和可还原编辑监控事件。
- **额度 / 订阅快照**：Claude Code 与 Codex CLI 的本地状态可写入额度和订阅快照数据面。

### 变更

- **有界本地扫描**：默认本地扫描使用近 30 天窗口，并按工具限制候选数、文件数、目录遍历数、JSONL/CSV 行数；文件游标缓存按工具、路径、大小、修改时间和扫描窗口记录，避免重复解析未变化来源。
- **小范围回填流程**：`--since/--until` 继续用于定向历史回填，避免默认执行全量递归扫描。
- **规范工具 key**：默认扫描只使用规范 key，避免同一本地路径被重复读取；显式 `--tool` 仍接受常见别名。
- **文档**：README、API、隐私、架构和路线图文档同步说明监控事件域、用量汇总域和当前工具支持边界。

### 修复

- 避免连续执行 `usage sync` 时重复读取未变化的本地会话记录和用量文件。
- 避免在大型本地工具数据目录中做无界递归扫描。
- 增加架构检查，防止扫描上限、缓存 schema、本地来源矩阵和 E2E 覆盖门禁回退。

### 测试 / CI

- Rust 客户端单测：**301 tests**。
- 客户端 E2E 矩阵门禁：v1.7 初版本地来源工具必须全覆盖，默认矩阵覆盖率要求 **100%**，并对可还原监控事件做字段级断言。
- 客户端 E2E 验证：未变化本地来源立即第二次执行 `usage sync` 时，解析 **0** 条 message 和 **0** 条 monitoring event。
- PR CI 门禁覆盖 Rust / Java / Go 构建与覆盖率、架构门禁、Java + Go E2E、Rust 客户端本地来源 E2E、Codecov、FOSSA 和自动 review 检查。

## [v1.6.3] — 2026-05-25

### 修复

- **Rust 客户端 — `provider` 字段语义修正**：`provider` 字段现在直接记录 agent 框架名（与 `tool` 字段相同，如 `claude` / `codex` / `cursor`）；移除了基于 base URL 推断 LLM 后端的逻辑，provider 与 tool 字段保持一致
- **Rust 客户端 — Codex 适配器 `file_paths[]` 向后兼容**：`CodexToolInput` 新增 `file_paths: Option<Vec<String>>` 字段，优先取 `file_paths[0]`，无则回退 `file_path` 单数字段
- **服务端 — dedup 重复检测**：`ValidationService` 新增步骤 2.5，60 秒窗口内 `(token_key, file_path, repo_url)` 相同记录标记为 `flagged("duplicate")`；Java 与 Go 两端同步实现
- **UTC 日期分桶统一**：客户端（Rust `chrono::Utc`）、Java 服务端（`Instant.now()`）、Go 服务端（`time.Now().UTC()`）三端统一使用 UTC 作为日期分桶基准

### 新增

- **Rust 客户端 — `backfill_repo_info`**：每次 `capture` 成功插入后，自动将当前 git 元数据（`repo_url`/`branch`/`current_sha`）回填到所有 `synced=0` 且 `repo_url` 为空的历史记录；使在 git 仓库外捕获的记录在后续有 git 上下文时能进入 flush 队列
- **Rust 客户端 — `init` 自动检测模式**：`aitrack init` 不传工具 flag 时自动检测 `~/.claude`、`~/.codex`、`~/.cursor` 目录存在性并安装对应钩子
- **Rust 客户端 — Claude 第三方 hook 冲突检测**：安装 Claude 钩子时若检测到 `settings.json` 中已有非 aitrack 的 PostToolUse hook command，通过 stderr 告警（安装不中止）
- **Rust 客户端 — Cursor 双注册**：Cursor 钩子现在同时写入 `hooks.postToolUse` 和 `hooks.afterFileEdit`；每个 entry 增加 `"matcher": "Write"` 和 `"timeout": 10` 字段；`remove_cursor_hook` 同步清理两个数组
- **`domain/provider.rs`**：新增 `infer_provider()` 函数统一 provider 字段赋值，直接返回 tool 名称

### 覆盖率

| 组件 | 测试数 | 行覆盖率 |
|------|--------|---------|
| Rust 客户端 | 252 | ≥ 90% |
| Java 服务端 | 226 | ≥ 90% |
| Go 服务端 | 244 | ≥ 90% |

---

## [v1.6.1] — 2026-05-21

### 变更

- **Go 服务端迁移为 PostgreSQL-only**：移除 `modernc.org/sqlite` 依赖，生产与 E2E 均需提供 `DATABASE_URL`；`testapp.MemoryConfig` 仅保留用于本地单元测试构造 chi router，不再用于 Docker E2E
- `docker/docker-compose.yml`：Go 服务容器改为 `DATABASE_URL` 连接 `db`（ParadeDB）服务，移除 `aitrack-go-data` SQLite 卷

### 修复

- E2E：`e2e/run.sh` Go 路径在 `pg_isready` 通过后新增 `sleep 2`，消除首次镜像拉取场景下 PostgreSQL 初始化竞态；新增容器日志抓取，方便排查 Go 服务端启动失败原因
- Go 服务端：测试占位符统一改为 `$N`（兼容 pgx/PostgreSQL）；覆盖率命令新增 `-coverpkg=./internal/...` 确保跨包覆盖统计准确；`page` 参数加 `clamp ≥ 0` 防止 `OFFSET` 为负

### 覆盖率

| 组件 | 测试数 | 行覆盖率 |
|------|--------|---------|
| Go 服务端 | 244 | 95.3% |

### 发行 / CI

- **多平台预构建二进制**：GitHub Release v1.6.1 发布全部 6 个平台的签名二进制：`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`、`aarch64-pc-windows-msvc`；每个二进制附带 `.sha256` 校验和和 ed25519 `.sig` 签名，供 `aitrack update` 验证
- **Claude AI Review 工作流**：修复 `claude-code-action@v1` post-step 错误，采用 GH_TOKEN PAT 鉴权，移除 `continue-on-error`；PR 提交后自动触发 AI 代码审查
- **移除 FOSSA 工作流**：免费账号已达项目数量上限，`.github/workflows/fossa.yml` 已删除

---

## [v1.6.0] — 2026-05-20

### 发布说明

v1.6.0 完成三端完整六边形架构重构，新增 `aitrack update` ed25519 更新命令，实现真实 HTTP 上报（非 stub），并引入基于 in-memory SQLite 的 E2E 真实链路测试。

### 新增

- **`aitrack update` 子命令**：ed25519 签名验证（硬编码公钥）；从 GitHub Releases API 拉取最新版本，下载二进制 + `.sig`，验签后原子替换当前可执行文件；全零占位公钥触发启动断言拒绝
- **关键词库防篡改检测**（Keyword tamper detection）：关键词以编译期常量硬编码；`keyword_fingerprint()` 计算 SHA256 并存入 `~/.aitrack/keywords.db`（WCDB 多库：`records.db` + `keywords.db`）；指纹不匹配时告警，二进制副本为权威来源
- **`server-go/testapp` 包**：导出 `Build()` + `MemoryConfig(adminKey)`，绕过 Go `internal` 访问限制；E2E 和集成测试可无外部进程启动真实 chi router + in-memory SQLite
- **真实链路 E2E 集成测试**（`e2e/chain_integration_test.go`）：`httptest.NewServer` 接入真实 Go chi router + in-memory SQLite；3 场景：完整 happy path（accepted=3）、篡改 `record_sig` → rejected（`sig_mismatch`）、无 Bearer token → 401
- **`domain/model/PageResult<T>`**（Java）：框架无关 `record PageResult<T>(List<T> content, long totalElements)`，替代 `EditRecordPort` 中的 Spring `Page<T>`；domain 层零 Spring 导入

### 变更

- **六边形架构三端全量落地**
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

v1.5.0 完成提示词捕获流水线：新增 `UserPromptSubmit` 钩子捕获用户提示词，`prompt_summary` 随编辑记录上报，服务端画像新增 `prompt_patterns` 意图分类维度。

### 新增

- **提示词捕获流水线**
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

v1.4.0 完成开发者 AI 工具使用画像：按需三维聚合（频率 / 深度 / 场景）+ 每日定时预热任务，Java 和 Go 双端功能完全对等。

### 新增

- **开发者 AI 工具使用画像**
  - Java `ProfileController`：`GET /api/v1/ai-track/profiles/{token_key}`，X-Admin-Key 鉴权
  - Java `ProfileService`：按需三维画像（使用频率 / 深度 / 场景 / 工具类型），`classifyScenario()` 路径启发式分类
  - Java `ProfileAggregationJob`：`@Scheduled(cron="0 0 2 * * *")` 每日凌晨预热
  - Go `ProfileHandler`：与 Java 功能完全对等，JSON schema 相同
  - 新增 `EditRecordRepository.findByTokenKeyAndStatusNot()` 和 `TokenRepository.findByTokenKeyAndActiveTrue()`
  - `AiTrackServerApplication` 添加 `@EnableScheduling`
  - `CONTRACT.md` §5 更新：画像端点完整 schema

### 文档

- `docs/PRIVACY.md`（两仓库同步）：数据采集透明度说明
- `CONTRACT.md` §5：画像端点 schema

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
