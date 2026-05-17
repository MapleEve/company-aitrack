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
| Go | 1.22+ | Go 服务端构建与测试 |
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
```

覆盖率门槛：LINE ≥ 90%，低于此值 Docker 构建会失败。

### 测试模块覆盖情况

| 模块 | 测试数 | 行覆盖率 |
|------|--------|----------|
| `adapters/claude.rs` | 9 | 98.6% |
| `adapters/codex.rs` | 9 | 99.3% |
| `adapters/cursor.rs` | 8 | 100% |
| `config.rs` | 17 | 83.9% |
| `crypto.rs` | 13 | 100% |
| `db.rs` | 15 | 91.7% |
| `diff.rs` | 12 | 100% |
| `git.rs` | 4 | 97.2% |
| `heartbeat.rs` | 9 | 97.4% |
| `init.rs` | 23 | 95.1% |
| `uploader.rs` | 12 | 99.0% |
| **TOTAL** | **140** | **87.75% 行 / 90.24% 函数** |

测试均为 `#[cfg(test)]` 内联模块。HTTP mock 使用 `wiremock`，临时文件使用 `tempfile`。

### Testkit 工厂

`src/testkit/factories.rs` 提供种子确定性的构建器：

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

# 运行（SQLite 默认存于 ./data/aitrack.db，端口 8080）
go run .

# 运行测试
go test -ldflags=-linkmode=external ./... -cover

# 在 Linux/Docker 内（无 Darwin dyld 问题）
go test ./... -coverprofile=cover.out
go tool cover -func=cover.out | tail -1

# 通过 Docker 构建
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

覆盖率门槛：total ≥ 90%，低于此值 Docker 构建会失败。

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

---

## E2E 测试

e2e 测试套件位于 `e2e/`，对 Java 和 Go 两套实现各跑一轮，证明协议兼容性。

### Go runner（模拟客户端）

```bash
# 从项目根目录
bash e2e/run.sh both   # Java + Go
bash e2e/run.sh java   # 仅 Java
bash e2e/run.sh go     # 仅 Go
```

脚本自动构建三个 Docker 镜像，启动服务端容器，运行测试，清理容器。

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

| 组件 | 工具 | 命令 | 门槛 |
|------|------|------|------|
| Rust 客户端 | cargo-llvm-cov | `cargo llvm-cov --summary-only` | LINE ≥ 90% |
| Java 服务端 | JaCoCo | `mvn verify` | LINE ≥ 90% |
| Go 服务端 | go cover | `go tool cover -func cover.out` | total ≥ 90% |

三个组件的 Docker 构建均内嵌覆盖率检查，不达标则构建失败。

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
