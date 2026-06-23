# AiTrack 协议合约（协议 v1.2，产品 v1.7.0）

本文是 `aitrack` Rust 客户端、Java 服务端和 Go 服务端共同遵守的协议合约。三端实现必须保持字段、签名算法、错误语义和数据域边界一致。

协议版本仍为 v1.2：`POST /admin/tokens` 签发单个 `credential` 字符串（`<token>-<hmac_secret>`），客户端按第一个 `-` 拆分后分别用于 Bearer token 和 HMAC 签名。

产品版本 v1.7.0 在不破坏协议 v1.2 的前提下扩展了三类能力：

- 原生编辑证据：Claude Code、Codex CLI、Cursor 具备原生编辑钩子适配器，可以生成带 diff、行数、仓库元数据和 `record_sig` 的 `EditRecord`。
- 动态状态心跳：`heartbeat.hooks` 从固定字段扩展为按工具 key 组织的动态状态图。
- 本地用量扫描：默认扫描 35 个工具 key，从本机会话目录、JSON/JSONL/NDJSON、CSV、SQLite 和本地客户端状态中提取用量、额度快照和可还原监控事件。

---

## 组件

| 组件 | 技术栈 | 协议职责 |
|------|--------|----------|
| `aitrack` client | Rust CLI | 安装钩子、捕获编辑事件、扫描本地来源、生成签名、上报数据 |
| `aitrack-server` | Java 17 + Spring Boot 3 | 管理 token、接收上报、执行校验链、提供查询 API |
| `aitrack-server-go` | Go + chi | 与 Java 端保持协议等价的服务端实现 |

---

## Credential

`POST /admin/tokens` 返回单个不透明凭据：

```text
credential = "<token>" + "-" + "<hmac_secret>"
```

- `token` 格式为 `aitrack_<hex>`，不包含 `-`。
- `hmac_secret` 是 HMAC 签名密钥。
- 客户端按第一个 `-` 拆分：前半段作为 `Authorization: Bearer`，后半段作为 `record_sig` 和 `X-AiTrack-Signature` 的 HMAC key。
- 响应体只返回 `{ "credential": "<token>-<hmac_secret>", "token_key": "<masked>" }`，不会分别返回 `token` 和 `hmac_secret`。
- 客户端配置只存储 `credential`，不会单独落盘两个字段；`hmac_secret` 不通过网络明文发送。

---

## 工具注册与数据域

aitrack 是通用、自托管、开源的员工 AI 编码监控与治理工具。协议明确分离三类数据，避免把用量统计伪装成编辑证据。

| 数据域 | 端点 / 载体 | 说明 |
|--------|-------------|------|
| `EditRecord` 监控事件 | `POST /api/v1/ai-track/edits` | 签名编辑证据，或本地会话记录中可还原的提示词、工具调用、窗口和编辑监控事件 |
| 工具状态心跳 | `POST /api/v1/ai-track/heartbeat`、`GET /api/v1/ai-track/devices` | `hooks` 是动态对象，key 为工具 key，value 表示本机可见或对应钩子可用 |
| 用量汇总与额度快照 | `/api/v1/ai-track/usage/*` | token、消息数、成本估算、本地额度和订阅快照等标量数据 |

纯 token 数、请求数、成本估算或本地额度信息只能进入 `/usage/*` 数据面，不能填充 `EditRecord` 字段后提交到 `/edits`。

### 当前工具支持范围

| 工具范围 | 原生编辑钩子 | 原生提示词钩子 | 本地会话扫描 | 用量汇总 | 额度 / 订阅快照 |
|----------|--------------|----------------|----------------------|----------|-----------------|
| `claude` | 是 | 是 | `.claude/`、projects / transcripts 目录、`~/.aitrack/sources/claude` | 是 | 本地限额快照 |
| `codex` | 是 | 否 | `.codex/sessions`、`~/.aitrack/sources/codex` | 是 | 本地会话限额快照 |
| `cursor` | 是 | 否 | Cursor globalStorage、`~/.aitrack/sources/cursor` | 是 | 否 |
| 默认本地扫描工具 | 否 | 否 | 明确的原生路径、应用状态、JSON/JSONL/NDJSON、CSV、SQLite，以及显式结构化导入根 | token、消息数、成本估算 | 否 |

默认本地扫描覆盖 35 个规范 key：

`claude`、`codex`、`cursor`、`trae`、`qwen`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode`。

显式 `--tool` 还接受 `roocode`、`kilo-code`、`gajae-code` 作为别名；默认扫描使用规范 key，避免重复读取同一类本地来源。

---

## 客户端命令

```text
aitrack init    [--claude] [--codex] [--cursor] [--tool <name> ...] [--api-url URL] [--credential CRED]
aitrack remove  [--claude] [--codex] [--cursor] [--tool <name> ...]
aitrack capture --tool <name>   (default: claude) [--api-url URL] [--credential CRED]
aitrack prompt-capture --tool <name>
aitrack inspect [--limit N] (default 20, max 200) [--pending] [--current-token]
aitrack stats
aitrack status
aitrack clean   [--all] [--force]
aitrack heartbeat
aitrack usage scan [--tool <name> ...] [--since YYYY-MM-DD] [--until YYYY-MM-DD]
aitrack usage sync [--tool <name> ...] [--api-url URL] [--credential CRED]
aitrack usage status
aitrack update
```

`prompt-capture` 只对具备原生提示词钩子的工具产生有效提示词上下文；v1.7.0 中该能力仅覆盖 `claude`。

---

## 本地存储

- 目录：`~/.aitrack/`
- `~/.aitrack/config.toml`：权限 `0600`，字段为 `api_url`、`credential`、`device_id`
- `~/.aitrack/records.db`：SQLite，权限 `0600`，存放 `EditRecord` 监控事件
- `~/.aitrack/usage.sqlite`：SQLite，存放本地来源级用量贡献、日聚合、额度快照、上传 outbox 和扫描游标缓存；正常扫描不长期保存逐条会话明细
- `device_id`：首次初始化时生成 UUIDv4，并持久化到 `config.toml`

### `records` 表

`records` 表只存放监控事件，不作为通用用量汇总表。

```sql
CREATE TABLE IF NOT EXISTS records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tool TEXT NOT NULL,
  tool_version TEXT,
  provider TEXT NOT NULL,
  model TEXT,
  session_id TEXT NOT NULL,
  repo_url TEXT NOT NULL DEFAULT '',
  branch TEXT NOT NULL DEFAULT '',
  current_sha TEXT NOT NULL DEFAULT '',
  file_path TEXT NOT NULL,
  added_lines INTEGER NOT NULL,
  removed_lines INTEGER NOT NULL,
  diff_hunk TEXT,
  metadata TEXT,
  synced INTEGER DEFAULT 0,
  synced_at TEXT,
  retry_count INTEGER DEFAULT 0,
  timestamp TEXT NOT NULL,
  token_key TEXT NOT NULL DEFAULT '',
  device_id TEXT NOT NULL DEFAULT '',
  hostname TEXT NOT NULL DEFAULT '',
  record_sig TEXT NOT NULL DEFAULT '',
  prompt_summary TEXT
);
CREATE INDEX IF NOT EXISTS idx_synced ON records(synced);
```

---

## Diff 算法

编辑行数必须使用 `similar` crate 的 Myers/LCS 最小 diff 计算：

- `added_lines`：实际新增行数
- `removed_lines`：实际删除行数
- `diff_hunk`：标准 unified diff，支持多 hunk

服务端按同一规则复核 diff 与行数，防止朴素行数统计被放大。

---

## `record_sig` 签名

每条记录写入本地 DB 时计算 `record_sig`，用于检测本地记录篡改与跨设备伪造。

```text
record_sig = HMAC_SHA256(
  key = hmac_secret,
  msg = token_key + "\n"
      + device_id + "\n"
      + hostname + "\n"
      + timestamp + "\n"
      + tool + "\n"
      + file_path + "\n"
      + repo_url + "\n"
      + current_sha + "\n"
      + added_lines (decimal) + "\n"
      + removed_lines (decimal) + "\n"
      + sha256_hex(diff_hunk if NULL use empty string "")
)
```

输出必须是小写十六进制字符串。字段顺序和 `\n` 分隔符必须在 Rust、Java、Go 三端字节一致。

---

## `POST /api/v1/ai-track/edits`

批量上报 `EditRecord` 监控事件。

```text
POST {api_url}/api/v1/ai-track/edits
Headers:
  Authorization: Bearer {token}
  Content-Type: application/json
  X-AiTrack-Device: {device_id}
  X-AiTrack-Client: aitrack/{version}
  X-AiTrack-Timestamp: {unix seconds}
  X-AiTrack-Signature: HMAC_SHA256(hmac_secret, "{X-AiTrack-Timestamp}\n{sha256_hex(body bytes)}")
```

### 请求体

```json
{
  "device_id": "<uuid>",
  "client_version": "1.0.0",
  "edits": [
    {
      "tool": "claude",
      "tool_version": "claude-code",
      "provider": "claude",
      "model": null,
      "session_id": "sess-abc123",
      "repo_url": "git@github.com:org/repo.git",
      "branch": "main",
      "current_sha": "a1b2c3d4e5f6",
      "file_path": "src/main.rs",
      "added_lines": 12,
      "removed_lines": 3,
      "diff_hunk": "@@ -10,7 +10,16 @@\n ...",
      "metadata": null,
      "timestamp": "2026-05-17T10:21:00Z",
      "device_id": "<uuid>",
      "hostname": "MacBook-Pro.local",
      "record_sig": "<hex>",
      "prompt_summary": "fix_debug"
    }
  ]
}
```

`edit` 对象包含 17 个必填字段和 1 个可选字段（`prompt_summary`）。`token_key` 不在请求体内，由服务端根据 Bearer token 推导。

### 响应体

```json
{
  "accepted": 3,
  "rejected": [{"index": 1, "reason": "invalid_sig"}],
  "flagged": [{"index": 2, "reason": "duplicate"}]
}
```

客户端处理约定：

- `accepted` 与 `flagged` 对应的本地行更新为 `synced=1, synced_at=now`
- `rejected` 对应的本地行执行 `retry_count += 1`
- 上传 SQL 条件包含 `retry_count < 5`

---

## `POST /api/v1/ai-track/heartbeat`

设备心跳用于报告客户端活跃状态、原生钩子状态和本机可见工具状态。

```json
{
  "device_id": "<uuid>",
  "hostname": "MacBook-Pro.local",
  "token_key_masked": "<masked>",
  "client_version": "1.0.0",
  "ts": 1747468800,
  "hooks": {
    "claude": true,
    "codex": false,
    "cursor": false,
    "opencode": true
  },
  "pending_count": 5
}
```

`hooks` 是动态对象。`claude`、`codex`、`cursor` 表示当前原生编辑钩子适配器状态；其他工具 key 表示注册、状态或本地用量来源可见性，不代表已经具备原生编辑钩子。

心跳在 `capture` 结束后节流发送（默认距上次超过 1 小时），`aitrack heartbeat` 可强制立即发送。

---

## `/api/v1/ai-track/usage/*`

用量端点只接收标量用量、消息数、成本估算和额度快照。

### `POST /api/v1/ai-track/usage/rollup`

```json
{
  "items": [
    {
      "device_id": "550e8400-e29b-41d4-a716-446655440000",
      "day": "2026-06-16",
      "agent": "codex",
      "model": "gpt-5",
      "account": "local",
      "tokens_in": 10,
      "tokens_out": 20,
      "tokens_cache_read": 3,
      "tokens_cache_write": 4,
      "tokens_reasoning": 5,
      "message_count": 2,
      "source_cost": 0.25
    }
  ]
}
```

服务端按 `(token_key, device_id, day, agent, model, account)` 幂等 upsert。

### `POST /api/v1/ai-track/usage/subscription`

```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "agent": "codex",
  "account": "local",
  "plan": "Pro",
  "quota_session_remaining": 70,
  "quota_weekly_remaining": 80,
  "quota_reset_at": "2026-06-16T10:00:00Z",
  "reader_status": "ok",
  "snapshotted_at": "2026-06-16T09:00:00Z"
}
```

服务端按 `(token_key, device_id, agent, account)` 幂等 upsert。

### `GET /api/v1/ai-track/usage/summary`

该端点只读，Bearer token 即可。常用查询参数：

| 参数 | 说明 |
|------|------|
| `token_key` | 可选；默认当前 token |
| `from_day` / `to_day` | 可选；`YYYY-MM-DD` |
| `agent` | 可选；按工具 key 过滤 |
| `limit` | 可选；默认 20，最大 100 |

---

## 钩子模板

### Claude Code (`~/.claude/settings.json`)

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "apply_patch|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "<abs aitrack path> capture --tool claude",
            "timeout": 10
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "<abs aitrack path> prompt-capture --tool claude",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

### Codex CLI (`~/.codex/config.toml`)

```toml
# aitrack
[[hooks.PostToolUse]]
matcher = "apply_patch|Edit|Write"

[[hooks.PostToolUse.hooks]]
type = "command"
command = "<abs aitrack path> capture --tool codex"
timeout = 10
```

### Cursor (`~/.cursor/hooks.json`)

```json
{
  "hooks": {
    "postToolUse": [
      {
        "command": "<abs aitrack path> capture --tool cursor",
        "matcher": "Write",
        "timeout": 10
      }
    ],
    "afterFileEdit": [
      {
        "command": "<abs aitrack path> capture --tool cursor",
        "matcher": "Write",
        "timeout": 10
      }
    ]
  }
}
```

安装和卸载操作必须幂等：安装时去重，卸载后清理空容器。

---

## 采集流程

1. 读取 stdin JSON。
2. 按 `--tool` 选择适配器；当前原生编辑适配器为 Claude Code、Codex CLI、Cursor，其他工具不会通过原生钩子直接生成文件编辑类 `EditRecord`。
3. 解析对应工具 payload。
4. 使用 Myers/LCS 计算 diff。
5. 通过 `git` 读取仓库元数据：`rev-parse --git-dir`、`remote get-url origin`、`branch --show-current`、`rev-parse HEAD`。
6. 读取 OS hostname。
7. 计算 `record_sig`。
8. 写入本地 DB，并执行 2 秒去重。
9. 对 `repo_url` 为空的未同步历史记录做非致命 git 元数据回填。
10. 上传未同步记录。
11. 发送节流心跳。

适配器解析失败时必须写本地日志，不能静默吞错。

---

## 加固点

| 编号 | 加固点 | 作用 |
|------|--------|------|
| H1 | `record_sig` HMAC | 防止本地 DB 记录被篡改 |
| H2 | `record_sig` 绑定 `device_id` 与 token | 防止跨设备伪造 |
| H3 | 动态心跳 | 发现原生钩子卸载和本机工具状态漂移 |
| H4 | Myers/LCS diff | 防止行数膨胀 |
| H5 | `(token, file_path)` 限流 | 防止刷量 |
| H6 | 解析失败日志 | 避免适配器错误被静默吞掉 |
| H7 | `repo_url` 白名单 | 防止仓库归属伪造 |
| H8 | `file_path` 合理性校验 | 防止路径注入 |

---

## 语义检索端点

以下端点仅在 PostgreSQL/ParadeDB 模式可用；嵌入式数据库模式返回 `501 Not Implemented`。鉴权方式为 `X-Admin-Key`。

### `GET /api/v1/ai-track/edits/search`

BM25 全文检索 `diff_hunk` 和 `prompt_summary`。

| 参数 | 必填 | 说明 |
|------|------|------|
| `q` | 是 | 检索文本 |
| `limit` | 否 | 最大结果数，默认 20，最大 100 |
| `token_key` | 否 | 按开发者过滤 |
| `repo` | 否 | 按仓库过滤 |

### `POST /api/v1/ai-track/edits/similar`

pgvector HNSW 向量相似搜索。

| 字段 | 必填 | 说明 |
|------|------|------|
| `embedding` | 是 | 384 维查询向量 |
| `limit` | 否 | 最大结果数，默认 10，最大 50 |
| `token_key` | 否 | 按开发者过滤 |
| `repo` | 否 | 按仓库过滤 |

---

## 开发者使用画像

`GET /api/v1/ai-track/profiles/{token_key}` 返回指定开发者的 AI 工具使用画像。鉴权方式为 `X-Admin-Key`。

响应包含：

- `frequency`：30 天日均、12 周周均和每日趋势。
- `depth`：单次编辑规模分布、P50/P90、注释密度。
- `languages`：按文件扩展名推断的语言分布。
- `tools`：按工具 key 统计的监控事件分布。
- `prompt_patterns`：基于有界 `prompt_summary` 的意图分类。

画像只计算 `ACCEPTED` 与 `FLAGGED` 记录，不包含 `REJECTED` 记录。
