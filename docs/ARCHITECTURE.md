# 系统架构

## 适用对象

这篇面向希望理解 AiTrack 整体设计的开发者、运维人员和安全审查者。它描述三个组件的职责划分、数据流向、协议版本和技术选型理由。

---

## 组件概览

AiTrack 由三个独立组件构成，通过 HTTP/JSON 协议通信，所有行为准则由 `CONTRACT.md` v1.1 统一约束。

```
┌─────────────────────────────────────────────────┐
│  AI 编码工具（Claude Code / Codex CLI / Cursor）  │
│  PostToolUse / afterFileEdit 钩子                │
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
│  Spring Boot 3 / H2 / PG  │  │  chi v5 / SQLite（纯 Go）  │
│  10 步校验链               │  │  10 步校验链（完全等价）     │
└────────────────────────────┘  └────────────────────────────┘
```

---

## 客户端：Rust CLI

**职责**：在 AI 工具的编辑事件触发时，捕获、签名、本地存储并上报一条编辑记录。

### 模块布局

```
src/
  main.rs        — 命令分发
  cli.rs         — clap 参数定义
  config.rs      — ~/.aitrack/config.toml 读写，token masking
  db.rs          — SQLite records 表 CRUD
  crypto.rs      — HMAC-SHA256，record_sig，请求签名
  diff.rs        — Myers/LCS diff（similar crate）
  git.rs         — spawn git 获取 repo 元数据
  init.rs        — 安装/移除钩子（Claude/Codex/Cursor）
  uploader.rs    — 刷新未同步记录到服务端
  heartbeat.rs   — 节流心跳 POST
  adapters/
    claude.rs    — 解析 Claude Code PostToolUse payload
    codex.rs     — 解析 Codex CLI postToolUse payload
    cursor.rs    — 解析 Cursor afterFileEdit payload
```

### 本地存储

- `~/.aitrack/config.toml`（0600）：api_url、token、device_id、hmac_secret
- `~/.aitrack/records.db`（0600）：SQLite，存储所有捕获记录

`device_id` 在首次运行时生成 UUIDv4 并持久化，后续只读。

---

## 数据流：从钩子触发到入库

```
1. AI 工具触发 PostToolUse/afterFileEdit 钩子
2. aitrack capture 从 stdin 读取 JSON
3. 按 --tool 选择适配器（claude/codex/cursor）解析 payload
4. 调用 similar crate 计算 Myers/LCS diff
   → added_lines, removed_lines, diff_hunk
5. spawn git 获取 repo 元数据
   → repo_url, branch, current_sha
6. 获取 OS hostname
7. 计算 record_sig
   → HMAC_SHA256(hmac_secret, canonical_string)
8. 2 秒去重窗口检查（同 session_id + file_path）
9. INSERT INTO records（synced=0）
10. flush_unsynced → POST /api/v1/ai-track/edits
    → 服务端 10 步校验链
    → 更新 synced/retry_count
```

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

## 协议 v1.1 概览

**v1.1 变更**：在编辑记录和心跳请求中新增 `hostname` 字段，记录上报机器的 OS hostname。

- `hostname` 不是访问控制机制，不做 per-token 隔离
- 同一 token 在多台机器使用是合法场景，`hostname` 用于人工审查时区分来源

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

| 维度 | Java（Spring Boot 3） | Go（chi v5）|
|------|-----------------------|-------------|
| 数据库 | H2（默认）/ PostgreSQL | SQLite（纯 Go，无 CGO）|
| 部署模型 | JRE + jar，适合现有 JVM 基础设施 | 单一二进制，distroless 镜像，适合极简容器 |
| ORM | Spring Data JPA / Hibernate | 原生 database/sql，无 ORM |
| 适用场景 | 已有 Java 技术栈的团队 | 偏好轻量容器或无 JVM 环境 |

两套实现共享同一个 e2e 测试套件（`e2e/`），以证明协议兼容性。

---

## 安全设计原则

- **最小权限**：客户端配置和数据库均以 0600 权限存储
- **防篡改**：每条记录计算 record_sig，服务端重新验证
- **防重放**：请求签名包含时间戳，服务端拒绝 300 秒以外的请求
- **防伪造**：record_sig 绑定 device_id + token_key，跨设备伪造无效
- **加密存储**：hmac_secret 在服务端以 AES-256-GCM 加密存储（生产环境）

详见 [SECURITY_MODEL.md](SECURITY_MODEL.md)。
