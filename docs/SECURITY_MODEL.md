# 安全模型

版本：v1.7.0 · 最后更新：2026-06-18

## 适用对象

这篇面向需要评估 aitrack 安全性的开发者、管理员和安全审查者。它说明数据从捕获到入库全链路的防护机制、已知边界和运维注意事项。

---

## 核心安全目标

1. **防篡改**：开发者无法在上报前修改本地 SQLite 中的记录而不被发现
2. **防伪造**：无法伪造其他 device/token 的记录
3. **防重放**：无法重复提交同一批请求
4. **防数据虚报**：无法通过朴素统计或构造数据夸大 added_lines
5. **防静默移除**：钩子被移除后服务端可在 1 小时内感知

---

## v1.7.0 支持边界

aitrack v1.7.0 的采集模型由三层组成：

- **原生编辑证据**：`claude`、`codex`、`cursor` 具备原生编辑适配器，可生成带 diff、行数、仓库信息和 `record_sig` 的编辑记录。
- **原生提示词钩子**：`claude`、`codex`、`cursor` 支持原生提示词钩子；其他工具的提示词、工具调用和窗口上下文只从稳定的本地会话、轨迹、导出或遥测来源提取。
- **动态状态心跳与本地用量扫描**：所有已登记工具都可以进入心跳状态图；默认本地扫描覆盖 30 个已验证来源的规范工具 key，用于提取用量标量、额度/订阅快照，以及本地来源中可还原的监控事件。

纯 token、额度或订阅快照只属于用量数据域，不能替代编辑记录，也不会生成 diff、行数或 `record_sig`。

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
- 重新计算 HMAC 并与 header 值**常量时间比对**（Java `MessageDigest.isEqual`，Go `subtle.ConstantTimeCompare`），降低计时侧信道风险

超出时间窗口的请求直接返回 401，不进入后续校验。

---

## 请求体与批量上限

服务端在反序列化请求体前执行两道前置限制：

- **请求体上限 8 MiB**：超出则直接返回 413，不进入校验链（Java `spring.servlet.multipart.max-request-size`；Go `http.MaxBytesReader`）。
- **`edits` 数组上限 500 条**：超出则返回 400，防止单次请求拖垮服务端。

---

## 服务端 10 步校验链详解

每批上报数据按顺序经过以下步骤，前三步失败则整批拒绝（401），步骤 4-9 失败粒度为单条记录：

| 步骤 | 校验内容 | 失败结果 | 防护点 |
|------|----------|----------|--------|
| 1 | Bearer token 存在且 active | 401 整批 | 基础鉴权 |
| 2 | `X-AiTrack-Timestamp` 与服务器时差 ≤ 300 秒 | 401 整批 | 防重放 |
| 3 | `X-AiTrack-Signature` HMAC 验证（常量时间比对） | 401 整批 | 请求完整性 |
| 4 | 每条 `record_sig` HMAC 验证（常量时间比对） | 单条 `rejected: sig_mismatch` | 防本地 DB 篡改 |
| 5 | `diff_hunk` 解析行数与 `added_lines`/`removed_lines` 偏差 ≤ 1 | 单条 `flagged: diff_inconsistent` | 防伪造 diff |
| 6 | `repo_url` 在白名单内（enforce=true 时） | 单条 `flagged/rejected: repo_unknown` | 防 repo 伪造 |
| 7 | `file_path` 不含 `..`，与 `repo_url` 路径逻辑一致 | 单条 `flagged: path_mismatch` | 防路径注入 |
| 8 | `added_lines ≤ max_added_lines`（默认 5000） | 单条 `flagged: oversized` | 防行数膨胀 |
| 9 | (token_key, file_path) 每小时记录数 ≤ rate_limit（默认 30） | 单条 `rejected: rate_limited` | 防刷量 |
| 10 | accepted + flagged 写入数据库 | — | 数据持久化 |

**flagged 与 rejected 的区别**：rejected 不入库，客户端重试；flagged 照常入库但打标，供管理员人工审查。

---

## Myers/LCS Diff 防虚报

客户端使用 `similar` crate 的 Myers/LCS 最小 diff 算法，计算真实的 `added_lines` 和 `removed_lines`。

- 防止朴素行数统计（如 before 行数 + after 行数）造成的人为膨胀
- `diff_hunk` 为标准 unified diff 格式，支持多 hunk
- 服务端步骤 5 重新解析 diff_hunk 验证行数一致性

---

## Credential 存储与 hmac_secret 加密

### Credential 签发与拆分

- `POST /admin/tokens` 响应返回单一 `credential` 字段（`<token>-<hmac_secret>`），**仅返回一次**，服务端不保存明文
- 客户端按**第一个 `-`** 拆分：前半部分为 `token`，后半部分为 `hmac_secret`；两部分均不单独落盘，`credential` 整体存入 `config.toml`

### Token 哈希存储

- 服务端存储 `sha256(token)`，不存明文
- `token_key` = 去掉 `aitrack_` 前缀后的 `first_6 + "…" + last_4`，用于日志和标识，不可逆回 token

### hmac_secret AES-GCM 加密

- 生产环境：`AITRACK_SECRET_KEY`（Base64 编码的 32 字节）→ AES-256-GCM 加密后存储
- 开发环境：未设置 `AITRACK_SECRET_KEY` 时，以 `plain:` 前缀明文存储（仅限开发）
- hmac_secret 必须明文可恢复（服务端需重计算 record_sig），加密存储保护数据库泄漏场景

---

## 客户端本地安全

- `~/.aitrack/config.toml`：文件权限 0600，包含 `credential`（合并凭据，含 token 和 hmac_secret）
- `~/.aitrack/records.db`：文件权限 0600，SQLite 本地记录库
- `device_id`：UUIDv4，首次运行生成，不可重置（除非删除 config.toml）
- **原子创建**：`config.toml` 和 `records.db` 均先写临时文件再原子 rename，避免写入中断留下权限为 0644 的半成品文件
- **stdin 上限**：`capture` 命令读取 stdin 时设有字节上限，防止超大 hook payload 占满内存

---

## 心跳检测

钩子可能被开发者手动从 AI 工具配置中移除，绕过监控。心跳机制提供被动检测：

- 每次 `capture` 结束时，若距上次心跳 >1 小时，自动发送心跳
- `aitrack heartbeat` 命令强制立即发送
- 心跳包含动态工具 key 到安装/可用状态的映射：`hooks.<tool>: true/false`
- 管理员通过 `GET /api/v1/ai-track/devices` 查看设备心跳状态

**检测延迟**：钩子移除后，最迟在下一次 capture（或 1 小时内）触发心跳更新，`last_seen` 停止更新。

---

## 已知边界与局限

| 边界 | 说明 |
|------|------|
| `provider` / `model` 字段客户端自报 | 服务端不验证这些字段的真实性，不应作为可信数据源 |
| `hostname` 不做访问控制 | hostname 仅供人工审查区分机器来源，不影响鉴权逻辑 |
| 完全停用工具 | 开发者卸载 AI 工具（而非仅移除钩子）时，不会产生心跳，无法检测 |
| 本地时钟篡改 | 开发者可修改系统时钟绕过 timestamp 校验，但 record_sig 仍会因数据篡改而失效 |
| repo_url 非强制白名单 | `enforce=false` 时未知 repo 只被 flagged 不被拒绝，需人工审查 |
| hmac_secret 明文存储于客户端 | config.toml 以 0600 保护，但本机 root 权限可读取；属于已知权衡 |

---

## 运维安全建议

1. **Admin 接口隔离**：生产环境中通过网络 ACL 或反向代理限制 `/admin/**` 的访问源
2. **定期轮换 hmac_secret**：通过重新签发 token 实现，旧 token 停用前需客户端重新 `aitrack init`
3. **监控 flagged 记录**：定期查询 flagged 记录，对 `diff_inconsistent` 和 `oversized` 进行人工判断
4. **监控设备 hooks 状态**：`GET /devices` 返回的原生钩子或已登记工具状态异常需主动跟进
5. **HTTPS 传输**：`api_url` 生产环境应使用 HTTPS，防止 hmac_secret 和 token 在传输中泄漏
