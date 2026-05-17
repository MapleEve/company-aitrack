# API 参考

## 适用对象

这篇面向需要与 AiTrack 服务端集成的开发者、管理员和运维人员。所有端点的字段定义与 `CONTRACT.md` v1.1 严格一致。

---

## 运维操作指引 — 装完服务端之后怎么用

> 本节是端到端的管理员操作流程。如只需查阅单个端点的字段定义，可直接跳到下方各端点章节。

### 角色与端点对应关系

| 角色 | 时机 | 调用的端点 |
|------|------|-----------|
| **管理员** | 服务端启动后 | `POST /admin/tokens` — 签发 token |
| **管理员** | 将 token 交给开发者 | 复制 `token` 和 `hmac_secret` 字段值 |
| **客户端（aitrack）** | 开发者每次编辑后自动触发 | `POST /api/v1/ai-track/edits` — 上报编辑记录 |
| **客户端（aitrack）** | 每次 capture 结束、距上次 >1h 自动触发 | `POST /api/v1/ai-track/heartbeat` — 上报钩子状态 |
| **管理员** | 查看团队 AI 用量 | `GET /api/v1/ai-track/stats` — 按维度统计 |
| **管理员** | 排查可疑设备 / 钩子状态 | `GET /api/v1/ai-track/devices` — 设备心跳列表 |
| **管理员** | 原始记录审计 | `GET /api/v1/ai-track/edits` — 分页查询 |

### 步骤一：启动服务端

```bash
# 生成随机密钥（每次部署重新生成，妥善保存）
export AITRACK_SECRET_KEY=$(openssl rand -base64 32)
export AITRACK_ADMIN_KEY=$(openssl rand -hex 32)

# 用 Docker Compose 启动（H2 嵌入式数据库，适合快速体验）
docker-compose up -d --build

# 验证服务健康
curl http://localhost:8080/actuator/health
# 期望响应: {"status":"UP"}
```

### 步骤二：签发开发者 token

每位开发者（或每个 CI pipeline）需要一个独立 token。

```bash
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"alice-macbook"}'
```

响应示例：

```json
{
  "token": "aitrack_abcdef1234567890abcdef1234567890",
  "hmac_secret": "c2VjcmV0LWJhc2U2NA==",
  "token_key": "abcdef…7890"
}
```

**注意**：`token` 和 `hmac_secret` 明文**仅此一次**出现在响应中，服务端不保存明文。请立即将这两个值安全地交给对应开发者，由其在安装 aitrack 客户端时填入。

### 步骤三：开发者安装客户端钩子

管理员将 `token` 和 `hmac_secret` 交给开发者后，开发者在自己的机器上执行：

```bash
aitrack init --claude \
  --api-url http://localhost:8080 \
  --api-token aitrack_abcdef1234567890abcdef1234567890 \
  --hmac-secret c2VjcmV0LWJhc2U2NA==

aitrack status   # 验证钩子已安装
```

此后，客户端在每次 AI 工具编辑文件后自动调用 `POST /edits` 上报数据，无需管理员手动操作。

### 步骤四：管理员查看团队实际用量

服务端收到数据后，管理员通过以下命令查看：

```bash
# 查看每个 token（对应开发者）的汇总数据
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer aitrack_abcdef1234567890abcdef1234567890"

# 查看设备心跳状态（silent=false 说明钩子正常；若某台机器 hooks.claude=false 需跟进）
curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer aitrack_abcdef1234567890abcdef1234567890"
```

> **说明**：`GET /stats` 和 `GET /devices` 只需 Bearer token，不需要 HMAC 签名头，管理员用手中任意一个有效 token 即可调用。如需使用管理员 key 直接查询，目前管理接口只提供签发端点，查询数据须使用开发者 token。

---

## 鉴权模型

AiTrack 使用两类鉴权机制：

| 接口类型 | 鉴权方式 |
|----------|----------|
| 管理接口 `/admin/**` | `X-Admin-Key` 请求头（值来自 `AITRACK_ADMIN_KEY` 环境变量） |
| 客户端接口 `/api/**` | Bearer token + HMAC 请求签名（五个 `X-AiTrack-*` 请求头） |

### 客户端请求公共头

所有 `/api/v1/ai-track/*` 端点（`GET /edits`、`GET /stats`、`GET /devices` 除外）均需携带：

| 请求头 | 格式 | 说明 |
|--------|------|------|
| `Authorization` | `Bearer {token}` | 完整 token 明文 |
| `X-AiTrack-Device` | UUIDv4 字符串 | 客户端 device_id |
| `X-AiTrack-Client` | `aitrack/{version}` | 如 `aitrack/1.0.0` |
| `X-AiTrack-Timestamp` | Unix 秒整数字符串 | 请求时间戳，服务端验证 ±300 秒 |
| `X-AiTrack-Signature` | 小写十六进制 | HMAC 请求签名，见下 |

**X-AiTrack-Signature 计算方式：**

```
X-AiTrack-Signature = lowercase_hex(
  HMAC_SHA256(hmac_secret, "{X-AiTrack-Timestamp}\n{sha256_hex(raw_body_bytes)}")
)
```

---

## 接口清单

| 分组 | 方法 | 路径 | 说明 |
|------|------|------|------|
| 管理 | POST | `/admin/tokens` | 签发新 token |
| 编辑 | POST | `/api/v1/ai-track/edits` | 批量上报编辑记录 |
| 编辑 | GET | `/api/v1/ai-track/edits` | 分页查询编辑记录 |
| 心跳 | POST | `/api/v1/ai-track/heartbeat` | 设备心跳上报 |
| 统计 | GET | `/api/v1/ai-track/stats` | 聚合统计 |
| 设备 | GET | `/api/v1/ai-track/devices` | 设备列表 |

---

## POST /admin/tokens

签发新 token。token 明文仅在此响应中出现一次，服务端只保存 `sha256(token)`。

**鉴权**：`X-Admin-Key: {admin_key}`

### 请求体

```json
{
  "owner": "alice",
  "note": "CI pipeline"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `owner` | string | 是 | token 所有者标识 |
| `note` | string | 否 | 备注说明 |

### 响应（200）

```json
{
  "token": "aitrack_abcdef1234567890abcdef1234567890",
  "hmac_secret": "base64string==",
  "token_key": "abcdef…7890"
}
```

| 字段 | 说明 |
|------|------|
| `token` | 完整 bearer token，**仅此一次明文返回**，请立即存入客户端 config.toml |
| `hmac_secret` | 用于 record_sig 和请求签名计算，**仅此一次明文返回** |
| `token_key` | masked 标识符，格式：去掉 `aitrack_` 前缀后的 `first_6 + "…" + last_4` |

### curl 示例

将 `$AITRACK_ADMIN_KEY` 替换为部署时设置的 `AITRACK_ADMIN_KEY` 环境变量值，或直接传入字面量。

```bash
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"alice-macbook"}'
```

**真实响应样例：**

```json
{
  "token": "aitrack_abcdef1234567890abcdef1234567890",
  "hmac_secret": "c2VjcmV0LWJhc2U2NA==",
  "token_key": "abcdef…7890"
}
```

将 `token` 和 `hmac_secret` 的值交给对应开发者，用于 `aitrack init` 命令。

**签发多个 token（团队场景）：**

```bash
# 为 bob 签发
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"bob","note":"bob-thinkpad"}'

# 为 CI 签发
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"ci-bot","note":"github-actions"}'
```

### 错误响应

| 状态码 | 说明 |
|--------|------|
| 401 | `X-Admin-Key` 缺失或错误 |
| 400 | 请求体字段缺失（如 `owner` 为空） |
| 503 | 服务端未配置 `AITRACK_ADMIN_KEY` |

---

## POST /api/v1/ai-track/edits

批量上报 AI 编码工具产生的编辑记录。服务端对每条 edit 依次执行 10 步校验链。

**此端点由 aitrack 客户端自动调用**，管理员通常无需手动调用。若需测试或排查，可参考下方的签名算法示例。

**鉴权**：Bearer token + X-AiTrack-* 签名头（见公共头）

### 请求体

```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "client_version": "1.0.0",
  "edits": [
    {
      "tool": "claude",
      "tool_version": "claude-code",
      "provider": "anthropic",
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
      "device_id": "550e8400-e29b-41d4-a716-446655440000",
      "hostname": "MacBook-Pro.local",
      "record_sig": "a3f2b1c4..."
    }
  ]
}
```

**edit 对象共 17 个字段**（`token_key` 不在其中，服务端从 Bearer token 推导）：

| 字段 | 类型 | 可空 | 说明 |
|------|------|------|------|
| `tool` | string | 否 | `claude` \| `codex` \| `cursor` |
| `tool_version` | string | 是 | 如 `claude-code` |
| `provider` | string | 否 | 如 `anthropic` |
| `model` | string | 是 | 模型名称，客户端自报，不可信 |
| `session_id` | string | 否 | AI 工具的会话标识 |
| `repo_url` | string | 否 | git remote origin URL |
| `branch` | string | 否 | 当前分支名 |
| `current_sha` | string | 否 | HEAD commit SHA |
| `file_path` | string | 否 | 被编辑文件路径（相对路径） |
| `added_lines` | integer | 否 | Myers/LCS 计算的实际新增行数 |
| `removed_lines` | integer | 否 | Myers/LCS 计算的实际删除行数 |
| `diff_hunk` | string | 是 | 统一 diff 格式，支持多 hunk |
| `metadata` | string | 是 | JSON 附加信息 |
| `timestamp` | string | 否 | ISO 8601，捕获时间 |
| `device_id` | string | 否 | UUIDv4，与外层 device_id 相同 |
| `hostname` | string | 否 | 上报机器的 OS hostname（v1.1 新增） |
| `record_sig` | string | 否 | HMAC-SHA256 小写十六进制，见下方签名算法 |

### record_sig 签名算法（可复现脚本）

`record_sig` 覆盖 11 个核心字段，字段顺序与分隔符必须与 CONTRACT.md v1.1 完全一致：

```
HMAC_SHA256(
  key  = hmac_secret,
  msg  = token_key + "\n"
       + device_id + "\n"
       + hostname  + "\n"
       + timestamp + "\n"
       + tool      + "\n"
       + file_path + "\n"
       + repo_url  + "\n"
       + current_sha + "\n"
       + added_lines (decimal) + "\n"
       + removed_lines (decimal) + "\n"
       + sha256_hex(diff_hunk)   ← diff_hunk 为 null 时用空字符串 "" 计算
)
```

以下 bash 脚本演示如何计算 `record_sig`（使用 openssl，无额外依赖）：

```bash
#!/usr/bin/env bash
# 演示 record_sig 计算，与 CONTRACT.md v1.1 严格一致
# 用法: bash compute_record_sig.sh

HMAC_SECRET="c2VjcmV0LWJhc2U2NA=="   # base64；实际值来自 POST /admin/tokens 响应
TOKEN_KEY="abcdef…7890"
DEVICE_ID="550e8400-e29b-41d4-a716-446655440000"
HOSTNAME_VAL="MacBook-Pro.local"
TIMESTAMP="2026-05-17T10:21:00Z"
TOOL="claude"
FILE_PATH="src/main.rs"
REPO_URL="git@github.com:org/repo.git"
CURRENT_SHA="a1b2c3d4e5f6"
ADDED_LINES="12"
REMOVED_LINES="3"
DIFF_HUNK="@@ -10,7 +10,16 @@\n ..."   # diff_hunk 为 null 时改为空字符串 ""

# 1. 计算 diff_hunk 的 SHA-256（diff_hunk 为 null 时对 "" 计算）
DIFF_SHA256=$(printf '%s' "$DIFF_HUNK" | openssl dgst -sha256 -hex | awk '{print $2}')

# 2. 拼接 canonical 消息（字段顺序必须与 CONTRACT.md 一致）
MSG="${TOKEN_KEY}
${DEVICE_ID}
${HOSTNAME_VAL}
${TIMESTAMP}
${TOOL}
${FILE_PATH}
${REPO_URL}
${CURRENT_SHA}
${ADDED_LINES}
${REMOVED_LINES}
${DIFF_SHA256}"

# 3. 计算 HMAC-SHA256，输出小写十六进制
RECORD_SIG=$(printf '%s' "$MSG" | openssl dgst -sha256 -hmac "$HMAC_SECRET" -hex | awk '{print $2}')

echo "record_sig: $RECORD_SIG"
```

**手工调用 POST /edits 的 curl 示例**（用于集成测试或排查；正常情况下客户端自动完成）：

```bash
# 先计算签名所需的时间戳和请求体哈希
TS=$(date +%s)
BODY='{"device_id":"550e8400-e29b-41d4-a716-446655440000","client_version":"1.0.0","edits":[{"tool":"claude","tool_version":"claude-code","provider":"anthropic","model":null,"session_id":"sess-abc123","repo_url":"git@github.com:org/repo.git","branch":"main","current_sha":"a1b2c3d4e5f6","file_path":"src/main.rs","added_lines":12,"removed_lines":3,"diff_hunk":"@@ -10,7 +10,16 @@\n ...","metadata":null,"timestamp":"2026-05-17T10:21:00Z","device_id":"550e8400-e29b-41d4-a716-446655440000","hostname":"MacBook-Pro.local","record_sig":"<用上方脚本预先计算>"}]}'

BODY_SHA256=$(printf '%s' "$BODY" | openssl dgst -sha256 -hex | awk '{print $2}')
HMAC_SECRET="c2VjcmV0LWJhc2U2NA=="
REQ_SIG=$(printf "${TS}\n${BODY_SHA256}" | openssl dgst -sha256 -hmac "$HMAC_SECRET" -hex | awk '{print $2}')

curl -s -X POST http://localhost:8080/api/v1/ai-track/edits \
  -H "Authorization: Bearer aitrack_abcdef1234567890abcdef1234567890" \
  -H "Content-Type: application/json" \
  -H "X-AiTrack-Device: 550e8400-e29b-41d4-a716-446655440000" \
  -H "X-AiTrack-Client: aitrack/1.0.0" \
  -H "X-AiTrack-Timestamp: $TS" \
  -H "X-AiTrack-Signature: $REQ_SIG" \
  -d "$BODY"
```

### 响应（200）

```json
{
  "accepted": 2,
  "rejected": [
    {"index": 1, "reason": "sig_mismatch"}
  ],
  "flagged": [
    {"index": 2, "reason": "oversized"}
  ]
}
```

| 字段 | 说明 |
|------|------|
| `accepted` | 正常接受的记录数 |
| `rejected` | 被拒绝的记录（含 index 和 reason），不入库，客户端 retry_count+1 |
| `flagged` | 被标记的可疑记录（含 index 和 reason），**仍入库**，供管理员审查 |

**rejected reason 枚举**：`sig_mismatch`、`rate_limited`、`malformed`

**flagged reason 枚举**：`diff_inconsistent`、`repo_unknown`、`path_mismatch`、`oversized`

**客户端处理约定**：
- `accepted` + `flagged` 索引：更新本地 `synced=1, synced_at=now`
- `rejected` 索引：本地 `retry_count += 1`
- 上传条件：`WHERE synced=0 AND retry_count < 5`

### 错误响应

| 状态码 | 说明 |
|--------|------|
| 401 | token 无效、时间戳超出 300 秒窗口、或请求签名验证失败 |
| 400 | 请求体格式错误（如 edits 为空数组） |

---

## GET /api/v1/ai-track/edits

分页查询已入库的编辑记录。

**鉴权**：Bearer token（无需 X-AiTrack-Signature）

### 查询参数

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `token_key` | string | — | 过滤指定 token（masked 格式，如 `abcdef…7890`） |
| `repo` | string | — | 按 repo_url 部分匹配过滤 |
| `page` | int | 0 | 页码，从 0 开始 |
| `size` | int | 20 | 每页大小，最大 100 |

### curl 示例

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

# 查询最近 20 条记录（默认分页）
curl -s "http://localhost:8080/api/v1/ai-track/edits" \
  -H "Authorization: Bearer $TOKEN"

# 按 token_key 过滤 + 分页
curl -s "http://localhost:8080/api/v1/ai-track/edits?token_key=abcdef%E2%80%A67890&page=0&size=50" \
  -H "Authorization: Bearer $TOKEN"

# 按 repo 部分匹配过滤（URL 编码斜杠）
curl -s "http://localhost:8080/api/v1/ai-track/edits?repo=org%2Frepo&page=0&size=20" \
  -H "Authorization: Bearer $TOKEN"
```

### 响应（200）

```json
{
  "total": 142,
  "page": 0,
  "size": 20,
  "items": [
    {
      "id": 1,
      "tool": "claude",
      "file_path": "src/main.rs",
      "added_lines": 12,
      "removed_lines": 3,
      "repo_url": "git@github.com:org/repo.git",
      "branch": "main",
      "timestamp": "2026-05-17T10:21:00Z",
      "flagged": false,
      "flag_reason": null
    }
  ]
}
```

---

## POST /api/v1/ai-track/heartbeat

设备心跳上报，用于检测钩子是否被静默移除。

**此端点由 aitrack 客户端自动调用**，在每次 `capture` 执行结束后（距上次超过 1 小时时）自动发送，或运行 `aitrack heartbeat` 时立即发送。管理员通常无需手动调用此端点。

**鉴权**：Bearer token + X-AiTrack-* 签名头（body 的签名方式与 edits 端点相同）

### 请求体

```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "hostname": "MacBook-Pro.local",
  "token_key_masked": "abcdef…7890",
  "client_version": "1.0.0",
  "ts": 1747468800,
  "hooks": {
    "claude": true,
    "codex": false,
    "cursor": false
  },
  "pending_count": 5
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `device_id` | string | UUIDv4 |
| `hostname` | string | OS hostname |
| `token_key_masked` | string | masked token 标识 |
| `client_version` | string | aitrack 客户端版本 |
| `ts` | integer | Unix 秒 |
| `hooks` | object | 各工具钩子安装状态（true=已安装） |
| `pending_count` | integer | 本地未同步记录数 |

### 响应（200）

```json
{"ok": true}
```

---

## GET /api/v1/ai-track/stats

聚合统计查询。这是管理员查看团队 AI 编码效能的主入口。

**鉴权**：Bearer token（无需 X-AiTrack-Signature）

### 查询参数

| 参数 | 可选值 | 说明 |
|------|--------|------|
| `group_by` | `token` \| `repo` \| `device` \| `hostname` | 聚合维度 |

### curl 示例

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

# 按 token（开发者）维度汇总 — 最常用，用于效能月报
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"

# 按仓库维度汇总 — 用于了解哪个项目 AI 编辑最多
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=repo" \
  -H "Authorization: Bearer $TOKEN"

# 按 device 维度汇总 — 用于识别同一开发者多台机器的分布
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=device" \
  -H "Authorization: Bearer $TOKEN"

# 按 hostname 维度汇总 — 用于人工排查多设备共用 token 情况
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=hostname" \
  -H "Authorization: Bearer $TOKEN"
```

### 响应示例（group_by=token）

```json
[
  {
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 142,
    "added_lines": 3820,
    "removed_lines": 1240
  },
  {
    "token_key": "fedcba…0123",
    "owner": "bob",
    "edit_count": 87,
    "added_lines": 2310,
    "removed_lines": 890
  }
]
```

### 响应示例（group_by=repo）

```json
[
  {
    "repo_url": "git@github.com:org/backend.git",
    "edit_count": 198,
    "added_lines": 5200,
    "removed_lines": 1870
  },
  {
    "repo_url": "git@github.com:org/frontend.git",
    "edit_count": 31,
    "added_lines": 920,
    "removed_lines": 260
  }
]
```

### 响应示例（group_by=device）

```json
[
  {
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 98,
    "added_lines": 2700,
    "removed_lines": 830
  }
]
```

### 响应示例（group_by=hostname）

```json
[
  {
    "hostname": "MacBook-Pro.local",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 98,
    "added_lines": 2700,
    "removed_lines": 830
  },
  {
    "hostname": "alice-office-imac.local",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 44,
    "added_lines": 1120,
    "removed_lines": 410
  }
]
```

> 同一 `token_key` 出现多个不同 `hostname` 是正常情况（CONTRACT.md v1.1 明确支持一个 token 用于多台机器）。若某个 hostname 数据量异常偏高，可通过 `/devices` 进一步核查该设备的钩子状态。

---

## GET /api/v1/ai-track/devices

查询所有已上报心跳的设备列表及钩子状态。这是管理员排查异常设备（钩子被移除、心跳停止）的主要工具。

**鉴权**：Bearer token（无需 X-AiTrack-Signature）

### curl 示例

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

# 查询所有设备
curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN"
```

### 响应（200）

```json
[
  {
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "hostname": "MacBook-Pro.local",
    "client_version": "1.0.0",
    "last_seen": "2026-05-17T10:00:00Z",
    "hooks": {
      "claude": true,
      "codex": false,
      "cursor": false
    },
    "pending_count": 0,
    "silent": false
  },
  {
    "device_id": "661f9511-f30c-52e5-b827-557766551111",
    "token_key": "fedcba…0123",
    "owner": "bob",
    "hostname": "bob-thinkpad",
    "client_version": "1.0.0",
    "last_seen": "2026-05-10T08:30:00Z",
    "hooks": {
      "claude": false,
      "codex": false,
      "cursor": false
    },
    "pending_count": 23,
    "silent": true
  }
]
```

**字段说明：**

| 字段 | 说明 |
|------|------|
| `device_id` | 客户端 UUIDv4，首次 `aitrack init` 时自动生成 |
| `token_key` | 该设备使用的 masked token 标识 |
| `owner` | token 对应的所有者 |
| `hostname` | 上报机器的 OS hostname（v1.1 新增） |
| `client_version` | 客户端版本 |
| `last_seen` | 最后一次心跳时间 |
| `hooks.claude/codex/cursor` | 对应 AI 工具的钩子是否已安装 |
| `pending_count` | 客户端本地未同步的记录数（较大值提示上报异常） |
| `silent` | `true` 表示该设备所有钩子均已被移除，需人工跟进 |

**运维判断指南：**

- `hooks.claude=false`（且该开发者应使用 Claude Code）→ 钩子被移除或未安装，联系开发者重新执行 `aitrack init --claude`
- `silent=true` → 所有工具钩子均已移除，可能存在主动规避采集的行为
- `last_seen` 超过 3 天未更新（且开发者仍活跃）→ 客户端可能离线或崩溃
- `pending_count` 持续偏大 → 网络或鉴权异常，客户端数据积压无法上报
