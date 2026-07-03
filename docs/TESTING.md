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
  ├── testapp + in-memory SQLite（Go 单元/集成测试）
  └── HTTP 端到端（mock server / MockMvc）

E2E 测试
  ├── 真实 Docker 容器内运行（bash e2e/run.sh）
  ├── Java 和 Go 服务端各跑一轮
  ├── 覆盖从签发 Token 到统计查询的完整链路
  └── 真实链路测试（chain_integration_test.go）：Go router + in-memory SQLite，无 Docker
```

---

## 覆盖率门槛（90%）

| 组件 | 测量方式 | 当前门禁（2026-06-17） | 失败行为 |
|---|---|---|---|
| **Rust 客户端** | `cargo llvm-cov --summary-only` → 解析 `TOTAL` 行 | **LINE ≥ 90%** | 低于 90% 构建失败 |
| **Java 服务端** | JaCoCo `LINE COVEREDRATIO >= 0.90`（pom.xml `verify` 阶段） | **LINE ≥ 90%**（mvn verify） | 低于 90% 构建失败 |
| **Go 服务端** | `go tool cover -func cover.out` → 解析 `total` 行 | **95.3% total** | 低于 90% 构建失败 |

### 客户端覆盖率命令（Rust）

```bash
# 安装 cargo-llvm-cov（首次）
cargo install cargo-llvm-cov

# 生成覆盖率报告（控制台摘要）
cargo llvm-cov --summary-only

# 生成 lcov 格式（供 CI 上传）
cargo llvm-cov --lcov --output-path lcov.info

# 上传到 codecov
codecov -f lcov.info -F client
```

### Java 服务端覆盖率配置

在 `pom.xml` 中添加 JaCoCo 插件：

```xml
<plugin>
    <groupId>org.jacoco</groupId>
    <artifactId>jacoco-maven-plugin</artifactId>
    <version>0.8.13</version>
    <executions>
        <execution>
            <goals>
                <goal>prepare-agent</goal>
            </goals>
        </execution>
        <execution>
            <id>report</id>
            <phase>test</phase>
            <goals>
                <goal>report</goal>
            </goals>
        </execution>
    </executions>
</plugin>
```

运行并上传：

```bash
cd server-java
mvn test
# 覆盖率报告：target/site/jacoco/index.html
codecov -f target/site/jacoco/jacoco.xml -F server-java
```

### Go 覆盖率命令

```bash
cd server-go
go test ./... -coverprofile=coverage.out -covermode=atomic
go tool cover -func coverage.out
codecov -f coverage.out -F server-go
```

---

## 测试工厂模式（三语言）

所有工厂都遵循同一约定：

1. **确定性（Seed-based）**：给定相同种子，每次生成相同数据
2. **Builder 风格**：默认合法实例 + 字段级覆盖
3. **负例工厂**：明确命名的篡改方法（`tampered_*`, `Tampered*`）
4. **HMAC 内建**：工厂负责计算正确的 `record_sig`，保证默认实例通过签名验证

### Rust（`client/src/testkit/factories.rs` → `crate::domain::model::Record`）

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

### Java（`server-java/src/test/.../testkit/EditDtoFactory.java` → `domain/model` 包）

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

### Go（`server-go/internal/testkit/factory.go` → `domain/model` 包）

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

## 运行测试的命令

### Rust 客户端

```bash
cd client

# 全套单测
cargo test

# 运行特定模块
cargo test crypto::tests
cargo test diff::tests
cargo test db::tests

# 覆盖率（摘要）
cargo llvm-cov --summary-only
```

### Java 服务端

```bash
cd server-java

# 全套测试（含覆盖率门槛验证）
mvn verify

# 仅运行测试（不验证覆盖率）
mvn test

# 运行特定测试类
mvn test -Dtest=ValidationServiceTest
mvn test -Dtest=SignatureServiceCanonicalTest

# 查看覆盖率报告
open target/site/jacoco/index.html
```

### Go 服务端

```bash
cd server-go

# 全套测试
go test ./...

# 含覆盖率
go test ./... -coverprofile=cover.out
go tool cover -func cover.out
```

### E2E 测试

```bash
# Java + Go 各跑一轮 E2E
bash e2e/run.sh both

# 使用真实 Rust 二进制做端到端验证
bash e2e/run-client-e2e.sh both

# 真实链路集成测试（无 Docker，直接 Go test）
cd e2e && go test ./... -run TestReal -v
```

---

## 单元测试用例清单

### 客户端 crypto 模块

| 测试用例 | 验证内容 |
|----------|----------|
| `test_record_sig_canonical_order` | 字段顺序与协议规范一致 |
| `test_record_sig_null_diff_hunk` | diff_hunk 为 None 时使用空字符串的 SHA256 |
| `test_record_sig_changed_field_invalidates` | 修改任意字段后签名不同 |
| `test_request_sig_format` | `{ts}\n{sha256(body)}` 格式正确 |

### 客户端 diff 模块

| 测试用例 | 验证内容 |
|----------|----------|
| `test_diff_added_only` | 只新增行时 removed=0 |
| `test_diff_removed_only` | 只删除行时 added=0 |
| `test_diff_mixed` | 混合变更的行数准确 |
| `test_diff_no_change` | 内容相同时 added=removed=0 |
| `test_diff_large_file` | 大文件不超时、行数准确 |

### 客户端 adapters 模块

| 测试用例 | 验证内容 |
|----------|----------|
| `test_claude_adapter_valid` | 正常 Claude payload 解析成功 |
| `test_claude_adapter_missing_field` | 缺字段时返回错误（不 panic） |
| `test_codex_adapter_valid` | Codex payload 解析 |
| `test_cursor_adapter_valid` | Cursor payload 解析 |
| `test_adapter_parse_failure_logs_stderr` | 解析失败时 stderr 有日志输出 |

### 客户端 db 模块

| 测试用例 | 验证内容 |
|----------|----------|
| `test_insert_and_query` | 插入后可查询到记录 |
| `test_dedup_within_2s` | 2 秒内重复 (session_id, file_path) 不重复插入 |
| `test_dedup_after_2s` | 2 秒后相同 key 可插入 |
| `test_synced_flag_update` | 上传后 synced=1, synced_at 非空 |
| `test_retry_count_increment` | rejected 后 retry_count+1 |
| `test_upload_filter_retry_limit` | retry_count≥5 的记录不参与上传 |
| `test_backfill_repo_info_fills_empty` | backfill_repo_info 补填 repo_url 为空的未同步记录 |
| `test_backfill_repo_info_skips_synced` | backfill_repo_info 不修改 synced=1 的已上报记录 |
| `test_backfill_repo_info_skips_nonempty` | backfill_repo_info 不覆盖已有 repo_url 的记录 |

### 客户端 init 模块

| 测试用例 | 验证内容 |
|----------|----------|
| `test_install_claude_hook_idempotent` | 重复 init 不产生重复钩子 |
| `test_install_creates_file_if_missing` | 目标配置文件不存在时创建 |
| `test_remove_claude_hook` | remove 后钩子条目不存在 |
| `test_remove_nonexistent_hook_ok` | 钩子不存在时 remove 不报错 |
| `test_remove_cleans_empty_container` | 移除后空容器被清理 |
| `test_auto_detect_installs_found_tools` | 无工具标志时自动检测 ~/.claude/~/.codex/~/.cursor 并安装所有已找到的工具钩子 |
| `test_auto_detect_no_tools_found` | 未检测到任何工具目录时报错退出（不静默成功） |
| `test_claude_posttooluse_conflict_warning` | `init --claude` 检测到已有非 aitrack PostToolUse 钩子时向 stderr 输出警告 |
| `test_cursor_dual_registration` | Cursor 钩子写入 `postToolUse`、`afterFileEdit` 和 `beforeSubmitPrompt` 三个数组 |
| `test_remove_cursor_hook_cleans_both_arrays` | `remove_cursor_hook` 同时清理 `postToolUse`、`afterFileEdit` 和 `beforeSubmitPrompt` 三个数组 |

### Java HmacUtil

| 测试用例 | 验证内容 |
|----------|----------|
| `testRecordSigMatchesClientFormula` | 与 Rust 客户端同参数时结果一致 |
| `testRequestSigVerification` | 请求签名验证通过 |
| `testAes256GcmEncryptDecrypt` | AES 加密后能正确解密 |
| `testAes256GcmKeyLengthValidation` | 非 32 字节密钥时抛异常 |

### Java TokenService

| 测试用例 | 验证内容 |
|----------|----------|
| `testIssueToken_returnsPlaintext` | 签发时返回明文 token |
| `testIssueToken_storesHash` | 数据库只存 sha256(token) |
| `testTokenKeyMasking` | token_key 格式为 first6…last4 |
| `testHmacSecretEncrypted` | 数据库中 hmac_secret 为密文 |

### Java EditService 校验链

每个步骤至少 2 个测试：通过路径 + 失败路径。

| 步骤 | 通过测试 | 失败测试 |
|------|----------|----------|
| Step 1：token 验证 | `testValidToken` | `testInvalidToken_returns401` |
| Step 2：时间戳窗口 | `testTimestampWithinWindow` | `testTimestampExpired_returns401` |
| Step 3：请求签名 | `testRequestSigValid` | `testRequestSigMismatch_returns401` |
| Step 4：record_sig | `testRecordSigValid` | `testRecordSigTampered_rejected` |
| Step 5：diff 自洽 | `testDiffConsistent` | `testDiffInconsistent_flagged` |
| Step 6：repo 白名单 | `testRepoInWhitelist` | `testRepoUnknown_flagged` |
| Step 7：路径合理性 | `testFilePathNormal` | `testFilePathTraversal_flagged` |
| Step 8：超大改动 | `testNormalAddedLines` | `testOversizedLines_flagged` |
| Step 9：限流 | `testUnderRateLimit` | `testOverRateLimit_rejected` |
| Step 10：入库 | `testAcceptedEditPersisted` | — |

---

## 集成测试示例

### 客户端集成测试（Rust）

```rust
#[tokio::test]
async fn test_capture_upload_flow() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/ai-track/edits"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"accepted":1,"rejected":[],"flagged":[]})))
        .mount(&server).await;

    let config = Config { api_url: server.uri(), ... };

    let payload = r#"{"tool_use_id":"...","path":"src/main.rs","old":"","new":"fn main(){}"}"#;
    run_capture(payload, &config).await;

    let records = db.query_synced().await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].synced, 1);
}
```

### 服务端集成测试（Java，Spring Boot Test）

```java
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@AutoConfigureMockMvc
class EditIntegrationTest {
    @Test
    void testFullEditFlow() throws Exception {
        // 1. 签发 token
        String tokenResp = mockMvc.perform(post("/admin/tokens")
                .header("X-Admin-Key", "test-admin-key")
                .content("""{"owner":"alice","note":"test"}""")
                .contentType(APPLICATION_JSON))
            .andExpect(status().isOk())
            .andReturn().getResponse().getContentAsString();

        String token = extractToken(tokenResp);
        String hmacSecret = extractHmacSecret(tokenResp);

        // 2. 上报编辑记录（有效签名）
        EditRequest req = buildValidRequest(token, hmacSecret);
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .headers(buildValidHeaders(token, hmacSecret, req))
                .content(toJson(req)))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.accepted").value(1));

        // 3. 查询统计
        mockMvc.perform(get("/api/v1/ai-track/stats?group_by=token")
                .headers(buildValidHeaders(token, hmacSecret, "")))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$[0].editCount").value(1));
    }

    @Test
    void testTamperedRecordRejected() throws Exception {
        EditRequest req = buildTamperedRequest();
        mockMvc.perform(post("/api/v1/ai-track/edits")...)
            .andExpect(jsonPath("$.rejected[0].reason").value("sig_mismatch"));
    }
}
```

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
│  Dockerfile.server-go (golang:1.24)                    │
│    go mod download                                      │
│    go test ./... -coverprofile=cover.out               │
│    go tool cover → check total ≥ 90%                   │
│    CGO_ENABLED=0 go build -o aitrack-server            │
│  → COPY binary → distroless/base-debian12              │
└───────────────────────────────────────────────────────┘
```

### 本机验证命令

```bash
cd aitrack

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

## CI 流水线配置

```yaml
# .github/workflows/ci.yml（示意）
jobs:
  test-client:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-llvm-cov
      - run: cargo llvm-cov --lcov --output-path lcov.info
        working-directory: client
      - uses: codecov/codecov-action@v4
        with:
          files: client/lcov.info
          flags: client

  test-server-java:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with:
          java-version: '17'
      - run: mvn test
        working-directory: server-java
      - uses: codecov/codecov-action@v4
        with:
          files: server-java/target/site/jacoco/jacoco.xml
          flags: server-java

  test-server-go:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-go@v5
        with:
          go-version: '1.24'
      - run: go test ./... -coverprofile=coverage.out
        working-directory: server-go
      - uses: codecov/codecov-action@v4
        with:
          files: server-go/coverage.out
          flags: server-go
```

---

## 关键注意事项

- **Java 构建必须在 Docker 内进行**：本机无 JDK 17/Maven，`Dockerfile.server-java` 使用 `maven:3.9-eclipse-temurin-17` 镜像完成全部构建和测试。
- **Go 服务端无 CGO 依赖**：`modernc.org/sqlite` 已于 v1.6.1 移除，PostgreSQL 驱动 pgx 为纯 Go 实现；`CGO_ENABLED=0` 构建，无跨平台链接问题。
- **E2E 不修改真实编辑器配置**：所有操作在容器隔离环境中进行，不触碰 `~/.aitrack/`、`~/.claude/`、`~/.codex/`、`~/.cursor/` 等目录。
