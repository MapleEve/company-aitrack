# 统一测试工厂与覆盖率说明

本文档描述 aitrack 三个组件的测试体系、工厂模式、覆盖率门槛和 Docker 内验证流程。

---

## 三层测试架构

```
单元测试 (Unit)
  ├── 纯函数、业务逻辑、HMAC 规范值
  └── 不依赖网络/DB（wiremock/H2/SQLite-内存）

集成测试 (Integration)
  ├── 完整 Spring 上下文 + H2 内存库（Java）
  ├── 真实 SQLite（Go）
  └── HTTP 端到端（mock server / MockMvc）

E2E 测试
  ├── 真实 Docker 容器内运行
  ├── Java 和 Go 服务端各跑一轮
  └── 覆盖从签发 Token 到统计查询的完整链路
```

---

## 覆盖率门槛（90%）

| 组件 | 测量方式 | 失败行为 |
|---|---|---|
| **Rust 客户端** | `cargo llvm-cov --summary-only` → 解析 `TOTAL` 行 | 低于 90% 构建失败 |
| **Java 服务端** | JaCoCo `LINE COVEREDRATIO >= 0.90`（pom.xml `verify` 阶段） | 低于 90% 构建失败 |
| **Go 服务端** | `go tool cover -func cover.out` → 解析 `total` 行 | 低于 90% 构建失败 |

---

## 测试工厂模式（三语言）

所有工厂都遵循同一约定：

1. **确定性（Seed-based）**：给定相同种子，每次生成相同数据
2. **Builder 风格**：默认合法实例 + 字段级覆盖
3. **负例工厂**：明确命名的篡改方法（`tampered_*`, `Tampered*`）
4. **HMAC 内嵌**：工厂内部计算正确的 `record_sig`，保证默认实例通过签名验证

### Rust（`client/src/testkit/factories.rs`）

```rust
// 合法实例
let rec = EditRecordFactory::new(42).with_tool("claude").build();
let cfg = ApiConfigFactory::new(42).with_hmac_secret("secret").build();

// Payload JSON
let json = ClaudeHookPayloadFactory::new(1).build_json();
let json = CodexHookPayloadFactory::new(2).build_json();
let json = CursorHookPayloadFactory::new(3).build_json();

// 负例
let bad = tampered_record_sig(1);       // record_sig 被清零
let exp = tampered_expired_timestamp(1); // timestamp = 2000-01-01
let big = tampered_oversized_lines(1);  // added_lines = 99,999,999
```

### Java（`server-java/src/test/java/com/aitrack/server/testkit/`）

```java
// 合法实例
EditDto dto = EditDtoFactory.build();
EditDto dto = EditDtoFactory.with(e -> e.setTool("codex"));
EditDto dto = EditDtoFactory.buildForTool("cursor");

// 负例
EditDto bad = TamperedFactory.badRecordSig();
EditDto bad = TamperedFactory.oversizedAddedLines();
EditDto bad = TamperedFactory.nullTool();

// 上传请求
EditBatchRequest req = EditBatchRequestFactory.build(dto);

// Token
TokenEntity tok = TokenEntityFactory.build();
```

### Go（`server-go/internal/testkit/factory.go`）

```go
// 合法实例（函数选项风格）
tok := testkit.BuildToken()
dto := testkit.BuildEditDTO()
req := testkit.BuildUploadRequest(tok, dto)
hb  := testkit.BuildHeartbeatRequest()

// 负例
bad := testkit.TamperedEditDTO()
exp := testkit.ExpiredTimestampEditDTO()
big := testkit.OversizedEditDTO()
mal := testkit.MalformedEditDTO()
```

### E2E（`e2e/factory/factory.go`）

```go
// Payload 从 fixtures/prompts/ 提取的真实代码片段构造
p := factory.DefaultEditParams(seed, tok)
body := factory.BuildBatchRequest(deviceID, p.BuildDTO())
hb   := factory.BuildHeartbeatRequest(deviceID, tokenKey, pendingCount)

// 负例
tampered := factory.TamperedRecordSig(p)
oversized := factory.OversizedEdit(p)
missing   := factory.MissingFieldEdit(p)

// HMAC 规范串（与 CONTRACT.md 完全一致）
sig := factory.ComputeRecordSig(secret, tokenKey, deviceID, ...)
reqSig := factory.ComputeRequestSig(secret, unixTS, bodyBytes)
```

---

## E2E 场景清单

| 场景 | 覆盖的合约要求 |
|---|---|
| Admin token 鉴权 | 403 wrong key / 400 缺字段 / 200 正常签发 |
| 契约层验证 | 401 无 auth / 错 token / 过期 ts / 错签名；400 空 edits |
| 全链路 happy path | sign token → POST /edits → accepted=1 → GET → stats → devices |
| 防作弊链路 | 篡改 record_sig → rejected；oversized → flagged；缺字段 → rejected |
| 心跳链路 | POST /heartbeat → ok=true；devices 反映设备 |
| Repo 白名单 | enforce=false 时未知 repo 被接受或标记（不硬拒绝） |

---

## Docker 内验证流程

```
┌───────────────────────────────────────────────────────┐
│  Dockerfile.client (rust:1.82)                         │
│    cargo fetch --locked                                 │
│    cargo build --release --locked                       │
│    cargo test --locked                                  │
│    cargo llvm-cov → check TOTAL ≥ 90%                  │
│  → COPY aitrack binary → debian:bookworm-slim           │
└───────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────┐
│  Dockerfile.server-java (maven:3.9-eclipse-temurin-17) │
│    mvn dependency:go-offline                            │
│    mvn verify  ← JaCoCo LINE ≥ 90% gate inside here   │
│  → COPY .jar → eclipse-temurin:17-jre                  │
└───────────────────────────────────────────────────────┘
┌───────────────────────────────────────────────────────┐
│  Dockerfile.server-go (golang:1.22)                    │
│    go mod download                                      │
│    go test ./... -coverprofile=cover.out               │
│    go tool cover → check total ≥ 90%                   │
│    CGO_ENABLED=0 go build -o aitrack-server            │
│  → COPY binary → distroless/base-debian12              │
└───────────────────────────────────────────────────────┘
```

### 本机验证命令

```bash
cd aitrack-selfbuilt

# 客户端
docker build -f docker/Dockerfile.client -t aitrack-client:latest . 2>&1 | tail -20

# Java 服务端
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest . 2>&1 | tail -20

# Go 服务端
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest . 2>&1 | tail -20

# E2E（Java + Go 各一轮）
bash e2e/run.sh both
```

---

## 关键注意事项

- **Java 构建必须在 Docker 内进行**：本机无 JDK 17/Maven，`Dockerfile.server-java` 使用 `maven:3.9-eclipse-temurin-17` 镜像完成全部构建和测试。
- **Go 测试在 Linux 容器内无 CGO 问题**：`modernc.org/sqlite` 是纯 Go 实现，无需 cgo，`CGO_ENABLED=0` 构建。
- **E2E 不修改真实编辑器配置**：所有操作在容器隔离环境中进行，不触碰 `~/.aitrack/`、`~/.claude/` 等目录。
