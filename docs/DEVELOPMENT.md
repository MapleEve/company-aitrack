# 开发指南 / Development Guide

## 适用读者 / Who This Is For

本指南面向参与 AiTrack 开发的工程师。内容涵盖本地环境配置、各组件的构建与测试命令、覆盖率工具、E2E 测试执行，以及协议变更时的三端同步要求。

---

## 本地环境要求 / Local Environment Requirements

| 工具 | 版本 | 用途 |
|------|------|------|
| Rust / Cargo | stable（推荐 1.82+） | 客户端构建与测试 |
| JDK | 17+ | Java 服务端（本机未安装 JDK 时使用 Docker 构建） |
| Maven | 3.8+ | Java 服务端构建 |
| Go | 1.25+ | Go 服务端构建与测试 |
| Docker | 20+ | 跨平台构建、Java 构建、E2E 测试 |
| sqlite3 CLI | 任意版本 | E2E 测试时验证本地数据库 |
| git | 任意版本 | 客户端 git 元数据提取 |

**注意**：Java 服务端（Spring Boot 3.3.8）要求 JDK 17。若本机未安装 JDK，所有 Java 相关操作须在 Docker 内完成（见下文"通过 Docker 构建"）。

---

## 客户端（Rust）/ Client (Rust)

```bash
cd client/

# Debug 构建
cargo build

# Release 构建
cargo build --release

# 运行测试
cargo test

# 覆盖率测量（需先安装 cargo-llvm-cov）
cargo install cargo-llvm-cov
cargo llvm-cov --summary-only

# 覆盖率详情（HTML 报告）
cargo llvm-cov --open
```

覆盖率门槛：行覆盖 ≥ 90%。Docker 构建未达标时失败。

### CLI 命令 / CLI Commands

| 命令 | 说明 |
|------|------|
| `aitrack capture` | 从 stdin 解析钩子事件并记录编辑 |
| `aitrack prompt-capture` | 从 stdin 记录 UserPromptSubmit 事件 |
| `aitrack heartbeat` | 立即强制发送一次心跳 |
| `aitrack status` | 打印配置、device_id 和同步统计 |
| `aitrack inspect` | 查询本地 records.db |
| `aitrack init` | 初始化 config.toml 并安装钩子 |
| `aitrack update` | 下载并验证最新二进制（ed25519 签名） |

#### `aitrack update` — 自更新命令 / Self-Update Command

`aitrack update` 获取最新版本二进制并在替换当前运行的二进制前进行验证：

```
1. GET <api_url>/api/v1/ai-track/release/latest  → { version, download_url, signature_url }
2. Download binary to <tempfile>
3. Download detached ed25519 signature (.sig file)
4. Verify: ed25519::verify(PUBLIC_KEY_BYTES, sha256(binary), signature)
   → abort with error if verification fails
5. Atomic rename: tempfile → current binary path (via std::fs::rename)
```

ed25519 公钥在构建时编译进二进制（`include_bytes!`）。二进制被篡改或签名不匹配时直接中止，旧二进制不会被替换。

### Rust 客户端模块结构（Sprint 2 六边形）/ Rust Client Module Structure (Sprint 2 Hexagonal)

```
client/src/
├── main.rs / cli.rs / config.rs / lib.rs   — command dispatch, config, entry point
├── git.rs / init.rs / uploader.rs / heartbeat.rs / update.rs
│
├── domain/        ← 纯领域逻辑，零基础设施依赖
│   ├── model.rs   ← EditRecord, ApiConfig and other core domain models
│   ├── crypto.rs  ← HMAC-SHA256, record_sig, request signing
│   ├── diff.rs    ← Myers/LCS diff (similar crate)
│   └── keywords.rs ← Hardcoded keywords + SHA256 fingerprint
│
├── port/
│   ├── storage.rs ← StoragePort trait
│   └── upload.rs  ← UploadPort trait
│
├── adapter/
│   ├── sqlite/    ← SqliteStorage impl StoragePort
│   │   └── mod.rs / schema.rs / models.rs / queries.rs / vec.rs / keyword_store.rs
│   ├── http/      ← HttpUploader impl UploadPort (real HTTP POST)
│   │   └── mod.rs / upload.rs
│   └── event/     ← claude/codex/cursor adapters
│       └── mod.rs / claude.rs / codex.rs / cursor.rs
│
└── testkit/factories.rs   ← 种子确定性测试工厂
```

### 各模块测试覆盖率 / Test Coverage by Module

| 模块 | 测试数（Sprint 2） | 行覆盖率 |
|------|---------|---------|
| `domain/` | — | ≥ 90% |
| `port/` | — | ≥ 90% |
| `adapter/sqlite/`、`adapter/http/`、`adapter/event/` | — | ≥ 90% |
| `config.rs` / `git.rs` / `init.rs` / `uploader.rs` / `update.rs` / ... | — | ≥ 90% |
| **合计** | **291** | **90.71% 行覆盖** |

> Sprint 2 六边形架构重构后，测试随领域模块重新组织。总测试数从 143 增至 291。运行 `cargo llvm-cov --summary-only` 查看最新的各模块明细。

所有测试均为 `#[cfg(test)]` 内联模块。HTTP mock 使用 `wiremock`，临时文件使用 `tempfile`。

#### sqlite-vec（可选向量扩展）/ sqlite-vec (Optional Vector Extension)

`client/src/db/vec.rs` 模块在数据库打开时通过 `sqlite3_auto_extension` 注册 sqlite-vec。若扩展探针（`SELECT vec_version()`）失败，全局 `VEC_DISABLED` 置为 true，所有向量操作跳过——核心捕获流程不受影响。

验证 sqlite-vec 是否正常加载：
```bash
./target/debug/aitrack status   # 在 DEBUG 级别打印 "sqlite-vec loaded: v0.1.x"
```

`vec_records` 虚拟表（`vec0`，`float[384]`）在 vec 启用时自动创建。向量在 Phase DB-3 之前不会填充。

### Testkit 工厂 / Testkit Factories

`src/testkit/factories.rs` 提供种子确定性构建器：

```rust
// Valid instances
let rec = EditRecordFactory::new(42).with_tool("claude").build();
let cfg = ApiConfigFactory::new(42).with_hmac_secret("secret").build();

// Payload JSON
let json = ClaudeHookPayloadFactory::new(1).build_json();

// Negative cases (for anti-validation tests)
let bad = tampered_record_sig(1);        // record_sig zeroed
let exp = tampered_expired_timestamp(1); // timestamp = 2000-01-01
let big = tampered_oversized_lines(1);  // added_lines = 99,999,999
```

---

## Java 服务端 / Java Server

```bash
cd server-java/

# 运行测试（单元 + 集成，H2 内存库）
mvn test

# 运行测试 + 覆盖率验证（行覆盖 ≥ 90% 门槛）
mvn verify

# 启动开发服务器
mvn spring-boot:run
# → http://localhost:8080
# → H2 控制台: http://localhost:8080/h2-console
```

JaCoCo HTML 报告：`target/site/jacoco/index.html`

#### PostgreSQL / ParadeDB profile

```bash
# 使用 postgres profile 运行（需要 ParadeDB 在 localhost:5432 运行）
SPRING_PROFILES_ACTIVE=postgres mvn spring-boot:run
```

### 通过 Docker 构建（本机未安装 JDK 时）/ Building via Docker (when JDK is not installed locally)

```bash
# 从项目根目录运行
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .
```

构建过程中自动运行 `mvn verify`；覆盖率不足时构建失败。

### Testkit 工厂 / Testkit Factories

```java
// Valid instances
EditDto dto = EditDtoFactory.build();
EditDto dto = EditDtoFactory.with(e -> e.setTool("codex"));
EditDto dto = EditDtoFactory.buildForTool("cursor");

// Negative cases
EditDto bad = TamperedFactory.badRecordSig();
EditDto bad = TamperedFactory.oversizedAddedLines();
EditDto bad = TamperedFactory.nullTool();
```

---

## Go 服务端 / Go Server

```bash
cd server-go/

# 构建
go build ./...

# 运行（需 DATABASE_URL 环境变量，端口 8080）
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .

# 运行测试
go test -ldflags=-linkmode=external ./... -cover

# Linux/Docker 上运行（无 Darwin dyld 问题）
go test ./... -coverprofile=cover.out
go tool cover -func=cover.out | tail -1

# 通过 Docker 构建
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

覆盖率门槛：total ≥ 90%。Docker 构建未达标时失败。当前覆盖率：**95.3%**。

### testapp 包 — 进程内集成测试 / testapp Package — In-Process Integration Testing

`server-go/testapp/` 为需要真实 HTTP router 而不使用 Docker 的集成测试提供轻量级装配层：

```go
import "github.com/your-org/aitrack/server-go/testapp"

func TestChainIntegration(t *testing.T) {
    adminKey := "test-admin-key-32chars-xxxxxxxxxx"
    cfg := testapp.MemoryConfig(adminKey)   // in-memory test config, random port
    handler, cleanup, _ := testapp.Build(cfg)
    defer cleanup()

    srv := httptest.NewServer(handler)
    defer srv.Close()
    // ... hit srv.URL endpoints with http.DefaultClient
}
```

`MemoryConfig` 返回仅含 admin key 的 `config.Config`，绕过 Go 的 `internal` 包限制，使 `server-go/internal/` 之外的测试文件也能装配真实服务器。仅供本地单元测试使用，Docker E2E 使用真实 PostgreSQL 容器。

### Testkit 工厂 / Testkit Factories

```go
tok := testkit.BuildToken()
dto := testkit.BuildEditDTO()
req := testkit.BuildUploadRequest(tok, dto)
hb  := testkit.BuildHeartbeatRequest()

// Negative cases
bad := testkit.TamperedEditDTO()
exp := testkit.ExpiredTimestampEditDTO()
big := testkit.OversizedEditDTO()
```

#### 本地 ParadeDB 开发 / ParadeDB local dev

运行 Go 服务端对接本地 ParadeDB 实例：
```bash
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .
```
`DATABASE_URL` 为必填项，Go 服务端无内置嵌入式数据库回退模式。

---

## E2E 测试 / E2E Tests

E2E 测试套件位于 `e2e/`，分别针对 Java 和 Go 实现各运行一轮，验证协议兼容性。

### Go runner（模拟客户端）/ Go runner (simulated client)

```bash
# 从项目根目录运行
bash e2e/run.sh both   # Java + Go
bash e2e/run.sh java   # 仅 Java
bash e2e/run.sh go     # 仅 Go
```

脚本自动构建三个 Docker 镜像，启动服务端容器，运行测试，最后销毁容器。

### 真实 Rust 二进制 E2E / Real Rust Binary E2E

```bash
# 需要本地安装：cargo、sqlite3、curl、git、python3、uuidgen
bash e2e/run-client-e2e.sh both
```

测试使用临时 `AITRACK_HOME` 目录，不会触碰 `~/.aitrack/` 或 `~/.claude/`。

### docker-compose E2E（用于 CI）/ docker-compose E2E (for CI)

```bash
docker compose -f docker/docker-compose.e2e.yml --profile java up --abort-on-container-exit
docker compose -f docker/docker-compose.e2e.yml --profile go up --abort-on-container-exit
```

---

## 覆盖率汇总 / Coverage Summary

| 组件 | 工具 | 命令 | 门槛 | 当前值（v1.6.0） |
|------|------|------|------|-----------------|
| Rust 客户端 | cargo-llvm-cov | `cargo llvm-cov --summary-only` | 行覆盖 ≥ 90% | **90.71%** |
| Java 服务端 | JaCoCo | `mvn verify` | 行覆盖 ≥ 90% | **LINE ≥ 90%** |
| Go 服务端 | go cover | `go tool cover -func cover.out` | total ≥ 90% | **95.3%** |

三个组件的 Docker 构建均内嵌覆盖率检查，不达标则构建失败。

---

## 协议变更规则 / Protocol Change Rules

`CONTRACT.md` 是 Rust 客户端、Java 服务端和 Go 服务端共同遵守的单一可信来源（SSoT）。任何协议变更必须同步更新三端：

1. **更新 `CONTRACT.md`**：升级版本号，描述变更内容
2. **更新 Rust 客户端**：`crypto.rs`（record_sig 规范字符串）、对应适配器、uploader
3. **更新 Java 服务端**：`SignatureService`（规范字符串）、`EditDto`（字段）、相关测试
4. **更新 Go 服务端**：`service/signature.go`（规范字符串）、`model`（字段）、相关测试
5. **更新 E2E 工厂**：`e2e/factory/factory.go` 中的 `ComputeRecordSig`
6. **运行 E2E 套件**，验证三端兼容性

`record_sig` 规范字符串中的字段顺序和 `\n` 分隔符必须在三端字节完全一致。详见 `CONTRACT.md` 的记录签名章节。
