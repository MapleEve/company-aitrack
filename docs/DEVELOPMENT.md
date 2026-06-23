# 开发指南

## 适用对象

这篇面向参与 AiTrack 开发的工程师。它覆盖本地环境搭建、各组件的构建与测试命令、覆盖率工具、e2e 测试运行方式，以及协议变更时的三端同步要求。

---

## 本地环境要求

| 工具 | 版本要求 | 用途 |
|------|----------|------|
| Rust / Cargo | 稳定版（推荐 1.82+） | 客户端构建与测试 |
| JDK | 17+ | Java 服务端（若本机无 JDK，用 Docker 构建） |
| Maven | 3.8+ | Java 服务端构建 |
| Go | 1.24+ | Go 服务端构建与测试 |
| Docker | 20+ | 跨平台构建、Java 构建、e2e 测试 |
| sqlite3 CLI | 任意 | e2e 测试验证本地 DB |
| git | 任意 | 客户端 git 元数据提取 |

**注意**：Java 服务端构建依赖 JDK 17，若本机未安装，所有 Java 相关操作均需在 Docker 内进行（见下方"通过 Docker 构建"）。

---

## 客户端（Rust）

```bash
cd client/

# 构建（debug）
cargo build

# 构建（release）
cargo build --release

# 运行测试
cargo test

# 覆盖率测量（首次需安装 cargo-llvm-cov）
cargo install cargo-llvm-cov
cargo llvm-cov --summary-only

# 覆盖率明细（HTML 报告）
cargo llvm-cov --open

# 检查并安装最新版本（ed25519 签名验证）
./target/debug/aitrack update
```

覆盖率门槛：LINE ≥ 90%，低于此值 Docker 构建会失败。

### 客户端模块结构（v1.7）

客户端采用六边形架构。旧 `db/`、`adapters/`、顶层 `crypto.rs`、顶层 `diff.rs` 已删除，核心逻辑按领域、端口、适配器和本地来源扫描拆分：

```
client/src/
├── domain/          ← 纯领域逻辑，零基础设施依赖
│   ├── model.rs     ← InspectRow, Record（含 StatsRow）
│   ├── crypto.rs    ← HMAC-SHA256 签名
│   ├── diff.rs      ← Myers diff 解析
│   └── keywords.rs  ← 硬编码关键词 + classify_prompt() + keyword_fingerprint()
├── port/
│   ├── storage.rs   ← StoragePort trait
│   └── upload.rs    ← UploadPort trait
├── adapter/
│   ├── sqlite/      ← SqliteStorage implements StoragePort
│   ├── http/        ← HttpUploader implements UploadPort（真实 POST 逻辑）
│   └── event/       ← 原生编辑事件适配（claude / codex / cursor）
├── agent.rs         ← 动态工具注册表、默认扫描 key、别名和本地来源根目录
├── usage/           ← 本地用量扫描、用量汇总、额度快照、扫描游标缓存
├── update.rs        ← aitrack update 子命令（ed25519 签名验证）
└── testkit/         ← factories.rs（测试工厂，使用 domain::model）
```

v1.7.0 当前支持边界：

- 原生编辑钩子适配器：`claude`、`codex`、`cursor`。
- 原生提示词钩子：仅 `claude`。
- 默认本地扫描：35 个规范工具 key；显式 `--tool` 还接受 `roocode`、`kilo-code`、`gajae-code` 作为别名。
- 本地来源类型：本机会话目录、JSON/JSONL/NDJSON、CSV、SQLite 和本地客户端状态。
- 默认扫描窗口：近 30 天；显式 `--since/--until` 用于小范围回填；扫描游标缓存会跳过未变化来源。

### 测试模块覆盖情况

| 模块 | 覆盖目标 | 当前门禁 |
|------|----------|----------|
| `domain/` | 纯业务逻辑、HMAC、diff、关键词分类 | Rust 覆盖率 LINE ≥ 90% |
| `port/` | 存储 / 上传端口契约 | Rust 覆盖率 LINE ≥ 90% |
| `adapter/` | SQLite、HTTP、事件适配、本地用量来源 | Rust 覆盖率 LINE ≥ 90% |
| **TOTAL** | client 全量单测 + 覆盖率 | **301 tests；LINE ≥ 90%** |

> v1.7 本地来源扩展后，Rust 客户端单测覆盖动态工具注册表、用量汇总、会话记录监控、窗口化扫描和文件游标缓存。

测试均为 `#[cfg(test)]` 内联模块。HTTP mock 使用 `wiremock`，临时文件使用 `tempfile`。

### Testkit 工厂

`src/testkit/factories.rs` 提供种子确定性的构建器（基于 `domain::model` 类型）：

```rust
// 合法实例
let rec = EditRecordFactory::new(42).with_tool("claude").build();
let cfg = ApiConfigFactory::new(42).with_hmac_secret("secret").build();

// Payload JSON
let json = ClaudeHookPayloadFactory::new(1).build_json();

// 负例（用于反验证测试）
let bad = tampered_record_sig(1);       // record_sig 置零
let exp = tampered_expired_timestamp(1); // timestamp = 2000-01-01
let big = tampered_oversized_lines(1);  // added_lines = 99,999,999
```

### 关键词防篡改机制

`domain/keywords.rs` 中的关键词列表为硬编码，不可在本地修改。`keyword_fingerprint()` 返回当前关键词表的 SHA-256 摘要，上报时服务端可验证指纹一致性，防止本地关键词篡改影响分类准确性。

### update 子命令

```bash
./target/debug/aitrack update
```

从配置的更新服务器拉取最新版本元数据，验证 ed25519 签名后执行就地更新。签名验证失败时中止并报错，不执行替换。

### usage 子命令

```bash
# 扫描默认 35 个工具 key，只写入本地 usage.sqlite
./target/debug/aitrack usage scan

# 针对单个工具做小范围回填
./target/debug/aitrack usage scan --tool codex --since 2026-06-01 --until 2026-06-18

# 扫描、汇总并上传用量；本地会话记录中可还原的监控事件会进入 EditRecord 上报链路
./target/debug/aitrack usage sync --api-url http://localhost:8080 --credential <credential>

# 查看本地用量账本状态
./target/debug/aitrack usage status
```

`usage scan` 和 `usage sync` 默认使用近 30 天窗口。每个工具有候选数、文件数、目录遍历数和行数上限；未变化文件会被游标缓存跳过。

### sqlite-vec（可选向量扩展）

`client/src/adapter/sqlite/vec.rs` 在 DB 打开时通过 `sqlite3_auto_extension` 注册 sqlite-vec 扩展。若探测（`SELECT vec_version()`）失败，`VEC_DISABLED` 全局标志置为 `true`，所有向量操作跳过——核心捕获流程不受影响。

验证是否加载成功：
```bash
RUST_LOG=debug ./target/debug/aitrack status
# 应输出: sqlite-vec loaded: v0.1.x
```

`vec_records` 虚拟表（`vec0`，`float[384]`）在 vec 启用时自动创建。Embedding 填充在 Phase DB-3 完成。

---

## Java 服务端

```bash
cd server-java/

# 运行测试（unit + integration，H2 内存库）
mvn test

# 运行测试 + 覆盖率验证（LINE ≥ 90% 门槛）
mvn verify

# 启动开发服务器
mvn spring-boot:run
# → http://localhost:8080
# → H2 控制台：http://localhost:8080/h2-console
```

```bash
# 以 postgres profile 启动（需本地 ParadeDB/PostgreSQL）
SPRING_PROFILES_ACTIVE=postgres mvn spring-boot:run
```

JaCoCo HTML 报告：`target/site/jacoco/index.html`

### 通过 Docker 构建（无本机 JDK 时）

```bash
# 从项目根目录执行
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .
```

构建过程中自动执行 `mvn verify`，覆盖率不足则构建失败。

### Testkit 工厂

```java
// 合法实例
EditDto dto = EditDtoFactory.build();
EditDto dto = EditDtoFactory.with(e -> e.setTool("codex"));
EditDto dto = EditDtoFactory.buildForTool("cursor");

// 负例
EditDto bad = TamperedFactory.badRecordSig();
EditDto bad = TamperedFactory.oversizedAddedLines();
EditDto bad = TamperedFactory.nullTool();
```

---

## Go 服务端

```bash
cd server-go/

# 构建
go build ./...

# 运行（需设置 DATABASE_URL，端口 8080）
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .

# 运行测试
go test -ldflags=-linkmode=external ./... -cover

# 在 Linux/Docker 内（无 Darwin dyld 问题）
go test ./... -coverprofile=cover.out
go tool cover -func=cover.out | tail -1

# 通过 Docker 构建
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

覆盖率门槛：total ≥ 90%，低于此值 Docker 构建会失败。当前覆盖率：**95.3%**。

### testapp 包（E2E 和集成测试专用）

`server-go/testapp/` 提供内存 SQLite 配置与真实 chi router，供 E2E 和集成测试使用，无需真实数据库：

```go
// 返回内存 SQLite 配置（无外部依赖）
cfg := testapp.MemoryConfig("test-hmac-key")

// 返回真实 chi router，可直接传给 httptest.NewServer
router := testapp.Build(cfg)
srv := httptest.NewServer(router)
defer srv.Close()
```

`testapp.Build(cfg)` 与生产路径 `app.Build(cfg)` 使用相同组合根，区别仅在于数据库后端为内存 SQLite。这使得 E2E 测试可以验证真实 HTTP 链路行为，而无需外部进程或持久化文件。

### Testkit 工厂

```go
tok := testkit.BuildToken()
dto := testkit.BuildEditDTO()
req := testkit.BuildUploadRequest(tok, dto)
hb  := testkit.BuildHeartbeatRequest()

// 负例
bad := testkit.TamperedEditDTO()
exp := testkit.ExpiredTimestampEditDTO()
big := testkit.OversizedEditDTO()
```

#### ParadeDB 本地开发

```bash
# 以 postgres 模式启动 Go 服务端（需本地已运行 ParadeDB/PostgreSQL）
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .
```

Go 服务端需设置 `DATABASE_URL`，无内嵌 SQLite 回退（v1.6.1 已移除）。本地开发可通过 `docker-compose up db` 启动 ParadeDB，或仅用 `testapp.MemoryConfig` 做单元测试（不启动真实服务进程）。

---

## E2E 测试

e2e 测试套件位于 `e2e/`，对 Java 和 Go 两套实现各跑一轮，证明协议兼容性。六边形架构重构后新增基于 `testapp` 的真实链路 E2E 测试，无需外部服务进程。

### Go runner（模拟客户端）

```bash
# 从项目根目录
bash e2e/run.sh both   # Java + Go
bash e2e/run.sh java   # 仅 Java
bash e2e/run.sh go     # 仅 Go
```

脚本自动构建三个 Docker 镜像，启动服务端容器，运行测试，清理容器。

### 基于 testapp 的真实链路 E2E（推荐用于 Go 服务端）

`server-go/testapp/` 包提供内存 SQLite 配置与真实 chi router，E2E 测试直接启动真实服务端（无 Docker、无外部进程）：

```bash
# 在 server-go/ 目录运行所有测试（含 testapp E2E）
go test ./... -coverprofile=cover.out
```

典型 E2E 场景（`e2e/mock_chain_test.go`）：

- 正常 accepted：sig_match → POST /edits → 200 accepted=1
- sig_mismatch 拒绝：篡改 record_sig → rejected
- 未授权 401：无效 token → 401

所有场景均使用 `testapp.MemoryConfig("key")` + `testapp.Build(cfg)`，不要求真实 credential 或外部数据库。

### 真实 Rust 二进制 E2E

```bash
# 需要本机有 cargo、sqlite3、curl、git、python3、uuidgen
bash e2e/run-client-e2e.sh both
```

测试使用临时 `AITRACK_HOME` 目录，不触碰 `~/.aitrack/` 和 `~/.claude/`。

### docker-compose E2E（CI 用）

```bash
docker compose -f docker/docker-compose.e2e.yml --profile java up --abort-on-container-exit
docker compose -f docker/docker-compose.e2e.yml --profile go up --abort-on-container-exit
```

---

## 代码覆盖率汇总

| 组件 | 工具 | 命令 | 门槛 | 当前覆盖率 |
|------|------|------|------|------------|
| Rust 客户端 | cargo-llvm-cov | `cargo llvm-cov --summary-only` | LINE ≥ 90% | **301 tests；LINE ≥ 90%** |
| Java 服务端 | JaCoCo | `mvn verify` | LINE ≥ 90% | **LINE ≥ 90%** |
| Go 服务端 | go cover | `go tool cover -func cover.out` | total ≥ 90% | **95.3%** |

三个组件的 Docker 构建均内嵌覆盖率检查，不达标则构建失败。

---

## 本地钩子配置更新（重新激活）

`aitrack init --claude` 会将 aitrack 的钩子配置写入 `~/.claude/settings.json`（`hooks` 字段）。当 aitrack 发布新版本并更新钩子配置（例如新增 `UserPromptSubmit` 钩子用于提示词捕获）时，**已安装旧版本的用户必须重新执行 init 命令**，才能激活新版钩子。

```bash
# 重新激活最新钩子配置（安全：只写入 ~/.claude/settings.json，不影响本地代码库）
aitrack init --claude \
  --api-url <your-server-url> \
  --credential <your-credential>
```

**何时需要重新运行 init**：

| 变更类型 | 是否需要重新 init |
|----------|-----------------|
| 新增钩子事件类型（如 UserPromptSubmit） | 是 |
| 更新钩子命令模板 | 是 |
| 仅更新服务端代码 | 否 |
| 仅更新 CONTRACT.md 协议字段 | 否（除非 record_sig 规范变更） |

**验证钩子已激活**：

```bash
# 确认 ~/.claude/settings.json 中存在 UserPromptSubmit 钩子
grep -A 5 '"UserPromptSubmit"' ~/.claude/settings.json

# 检查 aitrack 当前配置与钩子状态
aitrack status
```

> 注意：`aitrack init --claude` 会覆盖 `~/.claude/settings.json` 中已有的 aitrack 钩子条目，但不会删除其他工具写入的钩子。

**自动检测模式**：不传任何工具 flag 时，`aitrack init` 会检测 `~/.claude`、`~/.codex`、`~/.cursor` 目录是否存在，对检测到的原生编辑适配工具自动安装钩子。若均未检测到，打印提示并退出。其他已登记工具可通过 `--tool <name>` 写入注册/状态路径，但不会因此获得原生编辑适配能力。

**第三方冲突告警**：Claude 安装时若发现 `settings.json` 中已存在非 aitrack 的 PostToolUse hook command，通过 stderr 警告用户，但不中止安装。

**Cursor 双注册**：Cursor 钩子同时写入 `hooks.postToolUse` 和 `hooks.afterFileEdit`，每个 entry 带 `matcher: "Write"` 和 `timeout: 10`，覆盖所有触发路径。

---

## 协议变更规则

`CONTRACT.md` 是客户端（Rust）、Java 服务端、Go 服务端三者共享的唯一真实来源。任何协议变更必须同步更新三端：

1. **更新 `CONTRACT.md`**：修改版本号，描述变更内容
2. **更新 Rust 客户端**：`crypto.rs`（record_sig canonical string）、对应 adapter、uploader
3. **更新 Java 服务端**：`SignatureService`（canonical string）、`EditDto`（字段）、相关测试
4. **更新 Go 服务端**：`service/signature.go`（canonical string）、`model`（字段）、相关测试
5. **更新 e2e 工厂**：`e2e/factory/factory.go` 中的 `ComputeRecordSig`
6. **运行 e2e 套件**验证三端兼容性

`record_sig` canonical string 的字段顺序和 `\n` 分隔符必须在三端字节一致。详见 `CONTRACT.md` 的 Record Signature 章节。
