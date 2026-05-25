# 安全模型

## 适用对象

这篇面向需要评估 AiTrack 安全性的开发者、管理员和安全审查者。它说明数据从捕获到入库全链路的防护机制、已知边界和运维注意事项。

---

## 核心安全目标

1. **防篡改**：开发者无法在上报前修改本地 SQLite 中的记录而不被发现
2. **防伪造**：无法伪造其他 device/token 的记录
3. **防重放**：无法重复提交同一批请求
4. **防数据虚报**：无法通过朴素统计或构造数据夸大 added_lines
5. **防静默移除**：钩子被移除后服务端可在 1 小时内感知

---

## 记录级签名：record_sig

record_sig 在每条记录写入本地 SQLite 时计算，服务端接收后重新验证。

### 计算方式

```
record_sig = lowercase_hex(
  HMAC_SHA256(
    key = hmac_secret,
    msg = token_key     + "\n"
        + device_id     + "\n"
        + hostname      + "\n"
        + timestamp     + "\n"
        + tool          + "\n"
        + file_path     + "\n"
        + repo_url      + "\n"
        + current_sha   + "\n"
        + added_lines   + "\n"   (十进制字符串)
        + removed_lines + "\n"   (十进制字符串)
        + sha256_hex(diff_hunk)  (diff_hunk 为 NULL 时取空字符串 "" 的 SHA256)
  )
)
```

**字段顺序和 `\n` 分隔符在客户端（Rust）、Java 服务端、Go 服务端三处必须字节一致。**

### 防护效果

| 攻击场景 | 为何失败 |
|----------|----------|
| 修改本地 DB 的 `added_lines` | token_key+device_id 绑定了签名，篡改后 record_sig 验证失败 → 服务端 `rejected: sig_mismatch` |
| 复制其他设备的记录 | record_sig 包含 device_id，换设备后签名不匹配 |
| 伪造不同 token 的记录 | record_sig 包含 token_key，token 不同则签名不匹配 |
| 修改 diff_hunk 夸大行数 | sha256(diff_hunk) 在签名覆盖范围内，修改会导致 sig_mismatch |

---

## 请求级签名：X-AiTrack-Signature

每次 HTTP 请求携带请求级签名，防止网络层重放攻击。

### 计算方式

```
X-AiTrack-Signature = lowercase_hex(
  HMAC_SHA256(hmac_secret, "{X-AiTrack-Timestamp}\n{sha256_hex(raw_body_bytes)}")
)
```

服务端校验：
- 验证 `X-AiTrack-Timestamp` 与服务器当前时间差 ≤ 300 秒（可配置）
- 重新计算 HMAC 并与 header 值常量时间比对

超出时间窗口的请求直接返回 401，不进入后续校验。

---

## 服务端 10 步校验链详解

每批上报数据按顺序经过以下步骤，前三步失败则整批拒绝（401），步骤 4-9 失败粒度为单条记录：

| 步骤 | 校验内容 | 失败结果 | 防护点 |
|------|----------|----------|--------|
| 0 | 请求体大小 ≤ 8 MiB，edits 数组 ≤ 500 条 | 413 / 400 整批 | 防过大请求（H5） |
| 1 | Bearer token 存在且 active | 401 整批 | 基础鉴权 |
| 2 | `X-AiTrack-Timestamp` 与服务器时差 ≤ 300 秒 | 401 整批 | 防重放（H2） |
| 3 | `X-AiTrack-Signature` HMAC 常量时间比对 | 401 整批 | 请求完整性，防 timing attack（H2） |
| 4 | 每条 `record_sig` HMAC 常量时间比对 | 单条 `rejected: sig_mismatch` | 防本地 DB 篡改（H1/H2） |
| 5 | `diff_hunk` 解析行数与 `added_lines`/`removed_lines` 偏差 ≤ 1 | 单条 `flagged: diff_inconsistent` | 防伪造 diff（H4） |
| 6 | `repo_url` 在白名单内（enforce=true 时） | 单条 `flagged/rejected: repo_unknown` | 防 repo 伪造（H7） |
| 7 | `file_path` 不含 `..`，与 `repo_url` 路径逻辑一致 | 单条 `flagged: path_mismatch` | 防路径注入（H8） |
| 8 | `added_lines ≤ max_added_lines`（默认 5000） | 单条 `flagged: oversized` | 防行数膨胀（H1/H4） |
| 9 | (token_key, file_path) 每小时记录数 ≤ rate_limit（默认 30） | 单条 `rejected: rate_limited` | 防刷量（H5） |
| 10 | accepted + flagged 写入数据库 | — | 数据持久化 |

**flagged 与 rejected 的区别**：rejected 不入库，客户端重试；flagged 照常入库但打标，供管理员人工审查。

---

## Myers/LCS Diff 防虚报（H4）

客户端使用 `similar` crate 的 Myers/LCS 最小 diff 算法，计算真实的 `added_lines` 和 `removed_lines`。

- 防止朴素行数统计（如 before 行数 + after 行数）造成的人为膨胀
- `diff_hunk` 为标准 unified diff 格式，支持多 hunk
- 服务端步骤 5 重新解析 diff_hunk 验证行数一致性

---

## Token 存储与 hmac_secret 加密

### Token 哈希存储

- 服务端存储 `sha256(token)`，不存明文
- Token 明文仅在签发时（`POST /admin/tokens` 响应）出现一次
- `token_key` = 去掉 `aitrack_` 前缀后的 `first_6 + "…" + last_4`，用于日志和标识，不可逆回 token

### hmac_secret AES-GCM 加密

- 生产环境：`AITRACK_SECRET_KEY`（Base64 编码的 32 字节）→ AES-256-GCM 加密后存储
- 开发环境：未设置 `AITRACK_SECRET_KEY` 时，以 `plain:` 前缀明文存储（仅限开发）
- hmac_secret 必须明文可恢复（服务端需重计算 record_sig），加密存储保护数据库泄漏场景

---

## 客户端本地安全

- `~/.aitrack/config.toml`：文件权限 0600，原子创建，包含 credential（合并的 token + hmac_secret）
- `~/.aitrack/records.db`：文件权限 0600，原子创建，SQLite 本地记录库
- 两个文件均先以 `O_EXCL` 原子写入再设置权限，消除 TOCTOU 竞争窗口
- `device_id`：UUIDv4，首次运行生成，不可重置（除非删除 config.toml）

---

## 心跳检测（H3）

钩子可能被开发者手动从 AI 工具配置中移除，绕过监控。心跳机制提供被动检测：

- 每次 `capture` 结束时，若距上次心跳 >1 小时，自动发送心跳
- `aitrack heartbeat` 命令强制立即发送
- 心跳包含各工具钩子安装状态：`hooks.claude/codex/cursor: true/false`
- Cursor 钩子注册于 `~/.cursor/hooks.json` 的 `postToolUse` 和 `afterFileEdit` 两个数组（双注册），任意一个数组的条目存在即视为已安装
- 管理员通过 `GET /api/v1/ai-track/devices` 查看设备心跳状态

**检测延迟**：钩子移除后，最迟在下一次 capture（或 1 小时内）触发心跳更新，`last_seen` 停止更新。

---

## 数据完整性补填：backfill_repo_info

每次 `capture` INSERT 完成后，客户端执行 `backfill_repo_info`（`adapter/sqlite/queries.rs`）：

```sql
UPDATE records
SET repo_url = ?, branch = ?, current_sha = ?
WHERE synced = 0 AND (repo_url = '' OR repo_url IS NULL)
```

**安全语义**：此步骤补填的是 `record_sig` 计算**之前**的字段，不修改已签名记录中的 `repo_url`。已有 `record_sig` 的记录不受影响；仅对 `synced=0` 且 `repo_url` 为空的历史记录做补填，确保因 git 不可用（如非 git 仓库目录）而遗漏 repo 上下文的记录在下次上报时能携带完整元数据。

> 注意：补填后的记录在上报前 `record_sig` 仍基于补填时的字段值重新计算，服务端校验照常进行。

---

## 已知边界与局限

| 边界 | 说明 |
|------|------|
| `provider` / `model` 字段客户端自报 | 服务端不验证这些字段的真实性，不应作为可信数据源 |
| `hostname` 不做访问控制 | hostname 仅供人工审查区分机器来源，不影响鉴权逻辑 |
| 完全停用工具 | 开发者卸载 AI 工具（而非仅移除钩子）时，不会产生心跳，无法检测 |
| 本地时钟篡改 | 开发者可修改系统时钟绕过 timestamp 校验，但 record_sig 仍会因数据篡改而失效 |
| repo_url 非强制白名单 | `enforce=false` 时未知 repo 只被 flagged 不被拒绝，需人工审查 |
| hmac_secret 明文存储于客户端 | config.toml 以 0600 保护，但本机 root 权限可读取；属于已知 trade-off |

---

## `aitrack update` ed25519 安全模型

`aitrack update` 命令实现了客户端自更新机制，采用 ed25519 签名验证确保二进制完整性。

### 公钥绑定

- 公钥以 Base64 编码硬编码在 Rust binary 中（编译时常量 `ED25519_PUBKEY_BASE64`）
- 客户端无法被外部修改公钥；攻击者无法通过替换签名文件绕过验证
- **安全断言**：`load_verifying_key()` 在解码后检查是否为全零字节，若是则 `anyhow::bail!`（防止占位公钥意外用于生产）
- **发布前必须**将占位公钥替换为真实 ed25519 公钥，否则 `aitrack update` 调用时直接报错

### 更新流程

```
aitrack update
  → GitHub Releases API（获取最新版本 tag 和资源列表）
  → 下载目标平台 binary（aitrack-<target>）
  → 下载对应签名文件（aitrack-<target>.sig，Base64 编码的 64 字节原始签名）
  → ed25519 验证：decode(sig_base64) → verifying_key.verify(binary_bytes, &sig)
  → 验证通过 → 原子替换（Unix: rename/inode 交换，保证不中断正在运行的进程）
  → 验证失败 → 中止，删除临时文件，报错退出
```

**签名格式**：64 字节 ed25519 原始签名，Base64 编码存储于 `.sig` 文件。  
**原子替换**（Unix）：先写入临时路径，再 `fs::rename()` 替换，inode 级别交换，不影响进程中已打开的旧二进制。  
**Windows**：原子替换待处理（Windows 不允许覆盖正在执行的文件，需延迟替换机制）。

---

## 关键词库防篡改

关键词库用于对 `prompt_summary` 中的提示词做意图分类（generate/fix_debug/refactor/explain/test/other）。

### 防篡改机制

- **硬编码**：关键词数组作为编译时常量嵌入 Rust binary（`domain/keywords.rs`），不可在运行时修改
- **指纹计算**：`keyword_fingerprint()` 对当前 binary 中的关键词常量数组计算 SHA256，得到 32 字节指纹
- **持久化存储**：指纹写入 `~/.aitrack/keywords.db`（独立于 `records.db` 的 WCDB 多库结构）
- **启动校验**：每次使用关键词分类前，重新计算当前 binary 的指纹并与 `keywords.db` 中记录比对；不一致时记录告警日志（binary 中的值为权威来源）

### 防护场景

| 场景 | 结果 |
|------|------|
| 用户手动修改 `keywords.db` 中的指纹 | 重计算后不匹配，告警；binary 中的关键词仍为准 |
| 攻击者替换 binary 中的关键词 | 需要重新编译并替换 binary，ed25519 签名验证阻止未签名 binary 的自动更新 |
| `keywords.db` 文件丢失 | 重新计算并写入，不影响捕获流程 |

### WCDB 多库结构

```
~/.aitrack/
  records.db   — 编辑记录（records + prompt_context + kv + vec_records）
  keywords.db  — 关键词指纹（keyword_fingerprint SHA256）
```

两库职责分离，避免关键词指纹校验与编辑记录存储相互干扰。

---

## 运维安全建议

1. **Admin 接口隔离**：生产环境中通过网络 ACL 或反向代理限制 `/admin/**` 的访问源
2. **定期轮换 hmac_secret**：通过重新签发 token 实现，旧 token 停用前需客户端重新 `aitrack init`
3. **监控 flagged 记录**：定期查询 flagged 记录，对 `diff_inconsistent` 和 `oversized` 进行人工判断
4. **监控设备 hooks 状态**：`GET /devices` 返回的 `hooks.claude=false` 设备需主动跟进
5. **HTTPS 传输**：`api_url` 生产环境应使用 HTTPS，防止 hmac_secret 和 token 在传输中泄漏
6. **发布前替换占位公钥**：确保 `ED25519_PUBKEY_BASE64` 为真实密钥，否则 `aitrack update` 无法使用
