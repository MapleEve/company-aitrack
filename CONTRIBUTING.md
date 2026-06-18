# 贡献指南

欢迎为 aitrack 提交改进。

### 基本原则

- 改协议字段（`CONTRACT.md`）必须同时更新三端：Rust 客户端、Java 服务端、Go 服务端
- 新改动必须通过测试并满足覆盖率门槛
- 不提交密钥、token、环境文件或私有路径

### 环境要求

| 组件 | 最低版本 |
|------|---------|
| Rust | stable（推荐最新 stable） |
| JDK | 17 |
| Maven | 3.8+ |
| Go | 1.24+ |
| Docker | 用于端到端集成验证 |

### 构建与测试

**Rust 客户端**

```bash
cd client
cargo build --release
cargo test
# 覆盖率（需先安装 cargo-llvm-cov）
cargo llvm-cov --summary-only
```

**Java 服务端**

```bash
cd server-java
# 单测 + 覆盖率检查（LINE ≥ 90% 才通过）
mvn verify
# 仅跑单测
mvn test
```

**Go 服务端**

```bash
cd server-go
go test ./...
go test ./... -coverprofile=coverage.out -covermode=atomic
go tool cover -func coverage.out
```

### 覆盖率门槛

| 组件 | 工具 | 门槛 |
|------|------|------|
| Rust 客户端 | cargo-llvm-cov | 函数覆盖率 ≥ 90% |
| Java 服务端 | JaCoCo | LINE 覆盖率 ≥ 90%（`mvn verify` 强制） |
| Go 服务端 | go tool cover | 总覆盖率 ≥ 90% |

提交前必须确保覆盖率门槛通过，不允许带失败或警告的测试合入。

### 仓库结构

```
company-aitrack/
├── CONTRACT.md          — 权威协议文档（客户端与服务端共同遵守）
├── client/              — Rust CLI 客户端
│   ├── src/
│   │   ├── main.rs      — CLI 入口
│   │   ├── lib.rs       — 可复用模块导出
│   │   ├── cli.rs       — clap 参数定义
│   │   ├── config.rs    — ~/.aitrack/config.toml 读写
│   │   ├── domain/      — 领域模型、HMAC、diff、关键词分类
│   │   ├── port/        — StoragePort / UploadPort 端口契约
│   │   ├── adapter/     — SQLite、HTTP、原生编辑事件适配
│   │   ├── agent.rs     — 动态工具注册表、默认扫描 key 和别名
│   │   ├── usage/       — 本地用量扫描、汇总、快照和游标缓存
│   │   ├── git.rs       — 调用 git 获取仓库元数据
│   │   ├── init.rs      — 钩子安装与卸载
│   │   ├── uploader.rs  — 刷新未同步记录到服务端
│   │   ├── heartbeat.rs — 节流心跳 POST
│   │   └── update.rs    — ed25519 自更新
│   └── src/testkit/
│       └── factories.rs — 种子确定性测试工厂
├── server-java/         — Java 服务端（Spring Boot 3 / JDK 17）
│   └── src/test/java/com/aitrack/server/
│       └── testkit/     — Java 测试工厂（EditDtoFactory 等）
└── server-go/           — Go 服务端（Go 1.24+）
```

### 协议契约 CONTRACT.md

`CONTRACT.md` 是客户端与所有服务端实现必须共同遵守的权威协议文档。

改动协议字段前必须理解以下规则：

1. 任何对上报字段（`edits` 数组字段、`record_sig` 规范字符串、`X-AiTrack-*` 头）的增删改都必须同时更新三端。
2. 字段顺序和 `\n` 分隔符在 `record_sig` 规范字符串中是协议规范，不可随意调整——客户端与服务端必须字节对齐。
3. 协议版本变更建议在 `CONTRACT.md` 顶部写明 `vX.X change` 摘要，便于三端同步时对照。
4. 服务端测试 `SignatureServiceCanonicalTest` 专门验证规范字符串与 `CONTRACT.md` 一致，协议变更后必须同步更新该测试的预期值。

### 各组件开发调试

**Rust 客户端**

```bash
cd client

# 开发构建
cargo build

# 运行单个测试（快速迭代）
cargo test crypto::tests

# 完整测试 + 覆盖率
cargo llvm-cov --summary-only

# 手动测试 capture（需要先 init 并指向本地服务端）
./target/debug/aitrack init --claude \
  --api-url http://localhost:8080 \
  --credential <credential>
./target/debug/aitrack status
./target/debug/aitrack inspect --limit 5
```

本地 SQLite 记录存于 `~/.aitrack/records.db`，可用 `sqlite3` 直接查询调试。

**Java 服务端**

```bash
cd server-java

# 启动（H2 文件数据库，开箱即用）
mvn spring-boot:run

# H2 控制台：http://localhost:8080/h2-console
# JDBC URL: jdbc:h2:file:./data/aitrack  密码为空

# 运行特定测试
mvn test -Dtest=ValidationServiceTest

# 覆盖率报告
mvn verify
open target/site/jacoco/index.html
```

切换 PostgreSQL 只需修改 `application.yml` 的 `spring.datasource` 块，无需改业务代码。

**Go 服务端**

```bash
cd server-go

go test ./...
go run .
```

### 测试工厂模式

aitrack 的三端测试都采用工厂模式，原则是：**所有测试数据通过工厂构建，不在测试体内手写字面量**。

**Rust 客户端（`src/testkit/factories.rs`）**

```rust
// 默认有效记录
let record = EditRecordFactory::new(42).build();

// 定制字段
let record = EditRecordFactory::new(42).with_tool("codex").build();

// 负例
let bad = tampered_record_sig(42);        // sig 被篡改
let bad = tampered_oversized_lines(42);   // 行数超限
let bad = malformed_json();               // 语法错误 JSON
```

**Java 服务端（`server-java/src/test/java/.../testkit/`）**

```java
// 默认有效实例
EditDto dto = EditDtoFactory.build();

// 构造器式覆盖（sig 绑定字段变更后须重算 record_sig）
EditDto dto = EditDtoFactory.with(e -> e.setToolVersion("claude-code-v2"));

// 负例
EditDto bad = TamperedFactory.badRecordSig();       // sig_mismatch
EditDto bad = TamperedFactory.oversizedAddedLines(); // oversized flag
EditDto bad = TamperedFactory.nullTool();            // malformed rejection
```

### E2E 端到端测试

全链路 E2E 涉及：Rust 客户端 → Java 或 Go 服务端 → 数据库存储 → 查询接口。

推荐用 Docker Compose 拉起服务端，再用真实的 `aitrack` 二进制执行 `capture` 并验证数据落库：

```bash
# 1. 构建 Rust 客户端
cd client && cargo build --release

# 2. 启动 Java 服务端（Docker 示例）
cd server-java
docker build -t aitrack-server-java .
docker run -p 8080:8080 aitrack-server-java

# 3. 初始化客户端并发起 capture
./client/target/release/aitrack init \
  --claude \
  --api-url http://localhost:8080 \
  --credential <credential>

# 4. 模拟 Claude hook 触发（stdin 传入 PostToolUse payload）
echo '<claude-posttooluse-json>' | \
  ./client/target/release/aitrack capture --tool claude

# 5. 验证记录已上传
./client/target/release/aitrack inspect --limit 5
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/v1/ai-track/edits
```

E2E 运行中注意 `X-AiTrack-Timestamp` 的 300 秒时间窗口——测试环境时钟不同步会导致 `401`。

### 安全相关注意事项

- `config.toml` 和 `records.db` 权限必须是 0600，调试时不要放宽。
- 测试数据中不要使用真实 credential。
- 公开 Issue / PR 中不要粘贴含有真实凭据的日志或请求体。
- `hmac_secret`（credential 的后半部分）在服务端以 AES-256-GCM 加密存储（`HmacSecretEncryptor`），调试时可查看 `application.yml` 中的加密密钥配置。

## 版本发布流程

aitrack 客户端通过 GitHub Releases 分发二进制，ed25519 签名保证更新可信性。

**版本策略：遵循 SemVer。**

| 变更类型 | 版本位 |
|----------|--------|
| 向后兼容的新功能或新接口 | minor |
| 修复、文档、构建或小幅优化 | patch |
| 破坏性协议或 API 变更 | major |

### 发布步骤

1. **修改版本号**：
   ```bash
   # 例：1.7.0 → 1.7.1
   # 编辑 client/Cargo.toml，修改 version = "1.7.1"
   ```

2. **更新 CHANGELOG.md** — 在 `## [Unreleased]` 下添加 `## [vX.Y.Z] — YYYY-MM-DD` 条目。

3. **Commit**：
   ```bash
   git add client/Cargo.toml CHANGELOG.md
   git commit -m "chore(release): bump version to vX.Y.Z"
   git push origin main
   ```

4. **Tag（触发 CI 发布）**：
   ```bash
   git tag -a "vX.Y.Z" -m "Release vX.Y.Z"
   git push origin "vX.Y.Z"
   ```

5. **确认 CI**：`release.yml` 自动为 4 个平台构建二进制，ed25519 签名，发布到 GitHub Releases。

### 签名密钥管理

- ed25519 公钥嵌入在 `client/src/update.rs` 的 `ED25519_PUBKEY_BASE64` 常量中。
- 对应私钥存储为 GitHub Secret `RELEASE_SIGNING_KEY`（32 字节原始格式的 base64 编码）。
- **禁止轮换密钥**：已发布的客户端使用嵌入的公钥验证更新；更换密钥会导致所有已部署客户端的 `aitrack update` 无法通过验证。

### 分支规范

- 主干：`main`
- 功能分支：`feat/<short-description>`
- 修复分支：`fix/<short-description>`
- 协议变更：`contract/<version>-<description>`（需注明涉及哪些端）

### Commit 规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/v1.0.0/)：

```
<type>(<scope>): <subject>

[body]
[footer]
```

常用 type：`feat` / `fix` / `refactor` / `test` / `docs` / `chore`

scope 示例：`client` / `server-java` / `server-go` / `contract` / `testkit`

### Pull Request 规范

- PR 描述说明改动范围和测试结论
- 涉及协议变更时，列出三端同步状态
- 不得绕过 CI 检查强制合并
