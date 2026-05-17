# 贡献者指南 / Contributor Guide

这篇文档面向想深入参与 aitrack 开发的贡献者，补充 `CONTRIBUTING.md` 中未展开的架构细节。

---

## 仓库结构

```
aitrack-selfbuilt/
├── CONTRACT.md          — 协议单一可信来源（客户端与服务端共同遵守）
├── client/              — Rust CLI 客户端
│   ├── src/
│   │   ├── main.rs      — 命令分发
│   │   ├── cli.rs       — clap 参数定义
│   │   ├── config.rs    — ~/.aitrack/config.toml 读写
│   │   ├── db.rs        — SQLite records 表 CRUD
│   │   ├── crypto.rs    — HMAC-SHA256、record_sig、request_sig
│   │   ├── diff.rs      — Myers/LCS 差分（similar crate）
│   │   ├── git.rs       — 调用 git 获取仓库元数据
│   │   ├── init.rs      — hook 安装与卸载
│   │   ├── uploader.rs  — 刷新未同步记录到服务端
│   │   ├── heartbeat.rs — 节流心跳 POST
│   │   └── adapters/    — Claude / Codex / Cursor payload 解析
│   └── src/testkit/
│       └── factories.rs — 种子确定性测试工厂
├── server-java/         — Java 服务端（Spring Boot 3 / JDK 17）
│   └── src/test/java/com/aitrack/server/
│       └── testkit/     — Java 测试工厂（EditDtoFactory 等）
└── server-go/           — Go 服务端（Go 1.22+）
```

---

## 协议契约 CONTRACT.md 的地位

`CONTRACT.md` 是客户端与所有服务端实现的**单一可信来源**。

**改动协议字段前必须理解以下规则：**

1. 任何对上报字段（`edits` 数组字段、`record_sig` 规范字符串、`X-AiTrack-*` 头）的增删改都必须同时更新三端。
2. 字段顺序和 `\n` 分隔符在 `record_sig` 规范字符串中是协议规范，不可随意调整——客户端与服务端必须字节对齐。
3. 协议版本变更建议在 `CONTRACT.md` 顶部写明 `vX.X change` 摘要，便于三端同步时对照。
4. 服务端测试 `SignatureServiceCanonicalTest` 专门验证规范字符串与 `CONTRACT.md` 一致，协议变更后必须同步更新该测试的预期值。

---

## 三组件各自的开发调试方式

### Rust 客户端

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
  --api-token aitrack_<token> \
  --hmac-secret <secret>
./target/debug/aitrack status
./target/debug/aitrack inspect --limit 5
```

本地 SQLite 记录存于 `~/.aitrack/records.db`，可用 `sqlite3` 直接查询调试。

### Java 服务端

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

### Go 服务端

```bash
cd server-go

go test ./...
go run .
```

---

## 测试工厂模式

aitrack 的三端测试都采用工厂模式，原则是：**所有测试数据通过工厂构建，不在测试体内手写字面量**。

### Rust 客户端（`src/testkit/factories.rs`）

工厂使用种子（seed）保证确定性，可重复复现失败场景：

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

### Java 服务端（`server-java/src/test/java/.../testkit/`）

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

---

## E2E 端到端测试

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
  --api-token <token> \
  --hmac-secret <secret>

# 4. 模拟 Claude hook 触发（stdin 传入 PostToolUse payload）
echo '<claude-posttooluse-json>' | \
  ./client/target/release/aitrack capture --tool claude

# 5. 验证记录已上传
./client/target/release/aitrack inspect --limit 5
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/v1/ai-track/edits
```

E2E 运行中注意 `X-AiTrack-Timestamp` 的 300 秒时间窗口——测试环境时钟不同步会导致 `401`。

---

## 安全相关注意事项

- `config.toml` 和 `records.db` 权限必须是 0600，调试时不要放宽。
- 测试数据中不要使用真实 token 或 hmac_secret。
- 公开 Issue / PR 中不要粘贴含有真实凭据的日志或请求体。
- `hmac_secret` 在服务端以 AES-256-GCM 加密存储（`HmacSecretEncryptor`），调试时可查看 `application.yml` 中的加密密钥配置。
