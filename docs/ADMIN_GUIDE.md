# aitrack 管理员操作手册

> **适用版本**：v1.2.0+
> **文件性质**：内部文档，仅限 Codeup 私有仓库，禁止发布至 GitHub
> **受众**：负责部署和日常运维 aitrack 的内部管理员

---

## 目录

1. [前置要求](#1-前置要求)
2. [Credential 管理](#2-credential-管理)
3. [仓库白名单管理](#3-仓库白名单管理)
4. [设备与心跳监控](#4-设备与心跳监控)
5. [效能数据查询](#5-效能数据查询)
6. [语义检索（ParadeDB 模式）](#6-语义检索paradedb-模式)
7. [异常排查](#7-异常排查)
8. [数据保留与清理](#8-数据保留与清理)
9. [安全操作清单](#9-安全操作清单)

---

## 1. 前置要求

### 1.1 环境变量清单

服务端启动前，以下环境变量必须正确注入。推荐做法是在仓库根目录创建 `.env` 文件（已在 `.gitignore` 中排除），通过 `export $(grep -v '^#' .env | xargs)` 加载。

#### 必填变量

| 变量名 | 用途 | 格式 | 示例值 |
|--------|------|------|--------|
| `AITRACK_ADMIN_KEY` | 管理接口（`/admin/**`）鉴权密钥，签发 credential 时作为 `X-Admin-Key` 请求头的值 | 64 字符十六进制字符串（32 字节随机）| `a3f2b1c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2` |
| `AITRACK_SECRET_KEY` | AES-256-GCM 密钥，用于加密存储每个 credential 中的 `hmac_secret`；丢失后无法解密已有数据 | base64 编码的 32 字节随机数 | `K7gQzR+vF2JpLmN8sYdXeB1oW4T6hU0iCkAn9PqM3DE=` |
| `AITRACK_DB_PASSWORD` | ParadeDB/PostgreSQL 连接密码（postgres profile 模式必填）| 强密码字符串，生产环境禁止使用默认值 `aitrack_secret` | `Str0ng#Passw0rd!2026` |

#### 选填变量（Java 服务端 postgres profile）

| 变量名 | 用途 | 格式 | 默认值 | 示例值 |
|--------|------|------|--------|--------|
| `SPRING_PROFILES_ACTIVE` | 激活 Spring Profile；设置为 `postgres` 时启用 ParadeDB 模式，同时开启 BM25/ANN 语义检索端点；不包含 `dev` 时自动禁用 H2 console | 字符串 | `default`（H2 模式）| `postgres` |
| `AITRACK_DB_HOST` | ParadeDB 主机名 | 主机名或 IP | `localhost` | `db`（Docker Compose 网络内服务名）|
| `AITRACK_DB_PORT` | ParadeDB 端口 | 数字 | `5432` | `5432` |
| `AITRACK_DB_NAME` | 数据库名 | 字符串 | `aitrack` | `aitrack` |
| `AITRACK_DB_USER` | 数据库用户名 | 字符串 | `aitrack` | `aitrack` |

#### 必填变量（Go 服务端）

| 变量名 | 用途 | 格式 | 默认值 | 示例值 |
|--------|------|------|--------|--------|
| `DATABASE_URL` | Go 服务端 PostgreSQL/ParadeDB 连接 DSN；**必填**，无内嵌 SQLite 回退（v1.6.1 移除） | PostgreSQL DSN | —（无默认值，未设置则启动失败） | `postgres://aitrack:Str0ng#Passw0rd@db:5432/aitrack?sslmode=disable` |

#### 选填变量（业务参数）

| 变量名 | 用途 | 格式 | 默认值 | 示例值 |
|--------|------|------|--------|--------|
| `AITRACK_REPO_WHITELIST` | 允许上报的 repo URL 前缀，逗号分隔；留空表示不启用白名单过滤 | 逗号分隔的 URL 前缀字符串 | 空（不限制）| `git@github.com:myorg/,https://github.com/myorg/` |
| `AITRACK_REPO_ENFORCE` | 是否强制拒绝白名单以外的 repo_url；`false` 时仅标记不拒绝 | `true` 或 `false` | `false` | `true` |
| `AITRACK_RATE_LIMIT` | 每（token, file_path）组合每分钟允许的最大上报条数 | 正整数 | `60` | `120` |
| `AITRACK_TIMESTAMP_WINDOW` | 请求时间戳与服务端时间的允许偏差（秒），超出返回 401 | 正整数（秒）| `300` | `300` |
| `AITRACK_MAX_ADDED_LINES` | 单条记录 `added_lines` 的上限，超出时标记为 `flagged: oversized` | 正整数 | `5000` | `5000` |

#### 生成密钥的标准命令

```bash
# AITRACK_ADMIN_KEY：64 字符十六进制，openssl 生成
openssl rand -hex 32

# AITRACK_SECRET_KEY：base64 编码 32 字节随机数，openssl 生成
openssl rand -base64 32
```

> ⚠️ **注意：** 两个密钥均只在生成时显示一次。必须在生成后立即存入密码管理工具（如 1Password、Vault）。`AITRACK_SECRET_KEY` 丢失后，所有已签发 credential 中的 `hmac_secret` 将无法解密，等效于所有 credential 全部失效。

### 1.2 服务健康冒烟命令

部署完成或重启后，按以下顺序验证服务正常运行。

```bash
# ─── Java 服务端健康检查 ─────────────────────────────────────────────
curl -s http://localhost:8080/actuator/health
# 期望返回: {"status":"UP"}

# ─── Go 服务端健康检查 ───────────────────────────────────────────────
# Go 无专用 health 端点，通过 stats 端点（需有效 token）确认服务在线
# 401 说明服务在运行，token 无效属正常
curl -s -o /dev/null -w "%{http_code}" \
  http://localhost:8081/api/v1/ai-track/stats?group_by=token \
  -H "Authorization: Bearer placeholder"
# 期望返回: 401

# ─── ParadeDB 健康检查 ───────────────────────────────────────────────
docker compose -f docker/docker-compose.yml exec db \
  pg_isready -U aitrack -d aitrack
# 期望返回: localhost:5432 - accepting connections

# ─── 管理接口可达性检查 ─────────────────────────────────────────────
curl -s -o /dev/null -w "%{http_code}" \
  -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: wrong-key" \
  -H "Content-Type: application/json" \
  -d '{"owner":"smoke"}'
# 期望返回: 401（服务在运行，admin key 错误属正常）
```

---

## 2. Credential 管理

### 2.1 签发 Credential

每位开发者（或每个 CI pipeline）需要一个独立 credential。Credential 包含 token 和 hmac_secret，由服务端合并为一个不透明字符串签发。

**前提**：`AITRACK_ADMIN_KEY` 环境变量已注入服务端。

```bash
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "alice",
    "note": "alice-macbook-2026"
  }'
```

**请求体字段说明：**

| 字段 | 类型 | 是否必填 | 说明 |
|------|------|----------|------|
| `owner` | string | 是 | token 所有者标识，建议用用户名或邮箱前缀，便于 stats 查询时识别 |
| `note` | string | 否 | 备注，如机器名、用途（ci-bot、alice-thinkpad） |

**返回示例：**

```json
{
  "credential": "aitrack_abcdef1234567890abcdef1234567890-c2VjcmV0LWJhc2U2NA==",
  "token_key": "abcdef…7890"
}
```

**返回字段说明：**

| 字段 | 说明 |
|------|------|
| `credential` | 合并凭据字符串，格式为 `<token>-<hmac_secret>`（按第一个 `-` 拆分）。**明文仅此一次出现，服务端不存储明文**，请立即安全地交给对应开发者。 |
| `token_key` | masked 标识符，格式为去掉 `aitrack_` 前缀后的 `前6位 + "…" + 后4位`，用于在 stats、devices 等查询响应中识别 token 身份，不含敏感信息。 |

**批量为团队签发示例：**

```bash
# 为多位开发者批量签发（建议逐个执行，立即交付 credential）
for member in alice bob charlie ci-bot; do
  echo "=== 签发 $member ==="
  curl -s -X POST http://localhost:8080/admin/tokens \
    -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"owner\":\"${member}\",\"note\":\"${member}-init\"}"
  echo ""
done
```

### 2.2 Credential 只显示一次

> ⚠️ **注意：** `credential` 字段在 `POST /admin/tokens` 的响应体中**仅出现一次**。服务端存储的是 `sha256(token)` 哈希，不保存 credential 明文。响应体关闭后无法再次查看同一 credential。

**正确处理流程：**

1. 调用 `POST /admin/tokens` 后立即复制 `credential` 值
2. 通过安全渠道（如加密消息、密码管理工具共享）传递给对应开发者
3. 开发者在自己机器上执行 `aitrack init --credential <credential>` 完成安装
4. 管理员记录 `token_key`（masked）与 `owner` 的对应关系，便于后续 stats 查询

**如果 credential 丢失：** 直接重新签发一个新的 credential 交给开发者，旧 credential 会自然被替换（开发者重新执行 `aitrack init --credential <新credential>`）。服务端的旧 token 记录仍然存在，历史数据不会丢失。

### 2.3 验证 Credential 是否有效

由开发者在自己机器上执行以下命令验证：

```bash
# 在开发者机器上执行
aitrack status
```

**正常输出示例：**

```
hooks: claude=true, codex=false, cursor=false
pending: 0 records
last_seen: 2026-05-19T10:00:00Z
api_url: https://aitrack.company.internal
```

**异常信号：**

- `connection failed` 或 `401 Unauthorized` → credential 已失效或 api_url 配置错误
- `hooks: claude=false` → 钩子未安装或已被移除，需重新执行 `aitrack init --claude`
- `pending: N records`（N 持续增加）→ 数据积压，可能是网络或鉴权问题

管理员也可通过 `GET /api/v1/ai-track/devices` 查看该设备的最后心跳时间和钩子状态（见第 4 节）。

### 2.4 Credential 泄露处置

> ⚠️ **注意：** 当前版本（v1.2.x）**没有单个 credential 吊销 API**。收到 credential 泄露报告后，按以下步骤处理。

#### 当前限制

服务端目前没有 `DELETE /admin/tokens/{id}` 接口，无法精准吊销单个 token。

#### 临时止损方案

**方案 A：更换 AITRACK_SECRET_KEY（影响全部 credential，用于严重泄露场景）**

更换 `AITRACK_SECRET_KEY` 会导致服务端无法解密所有已签发 credential 中的 `hmac_secret`，所有请求签名验证将失败，相当于全量吊销：

```bash
# 步骤 1：生成新的 AITRACK_SECRET_KEY
NEW_SECRET_KEY=$(openssl rand -base64 32)
echo "NEW_SECRET_KEY=$NEW_SECRET_KEY"
# 记录新密钥到密码管理工具

# 步骤 2：更新 .env 文件
# 将 AITRACK_SECRET_KEY 替换为新值

# 步骤 3：重启服务端（Java）
docker compose -f docker/docker-compose.yml restart aitrack-server-java

# 步骤 4：通知所有开发者重新获取 credential
# 管理员需为每位开发者重新签发 credential
```

> ⚠️ **注意：** 更换 `AITRACK_SECRET_KEY` 后，所有开发者的 aitrack 客户端都会立即报 401，**必须提前通知所有人准备重新初始化**，否则会造成大范围数据上报中断。

**方案 B：重命名 AITRACK_ADMIN_KEY（中断新签发，不影响已有 token 的使用）**

如果泄露的是 `AITRACK_ADMIN_KEY`（而非开发者 credential），只需轮换管理员密钥，不影响已在使用的开发者 credential：

```bash
# 步骤 1：生成新的 AITRACK_ADMIN_KEY
NEW_ADMIN_KEY=$(openssl rand -hex 32)

# 步骤 2：更新 .env 并重启
# 替换 AITRACK_ADMIN_KEY 后重启服务端
docker compose -f docker/docker-compose.yml restart aitrack-server-java
```

**后续操作：**

1. 确认泄露的 credential 对应的 `owner`（从之前记录的签发日志或 stats 数据中查找）
2. 联系该开发者更新 credential（重新签发 → 重新执行 `aitrack init`）
3. 观察服务端日志中是否有来自该 token 的异常请求（大量 `rate_limited`、异常 repo_url 等）

---

## 3. 仓库白名单管理

### 3.1 白名单工作原理

白名单通过两个环境变量共同控制：

- `AITRACK_REPO_WHITELIST`：允许上报的 repo URL **前缀**列表，逗号分隔。服务端在 10 步校验链的第 6 步（repo 白名单）检查每条 edit 记录的 `repo_url` 是否以列表中任意一个前缀开头。
- `AITRACK_REPO_ENFORCE`：控制白名单检查的处置动作。

**两个变量的组合行为：**

| `AITRACK_REPO_WHITELIST` | `AITRACK_REPO_ENFORCE` | 行为 |
|--------------------------|------------------------|------|
| 空（未设置）| 任意值 | 白名单检查完全跳过，所有 repo_url 均接受 |
| 已设置 | `false`（默认值）| 不在白名单的记录**入库但标记** `flagged: repo_unknown`，不拒绝 |
| 已设置 | `true` | 不在白名单的记录**直接拒绝**（返回 `rejected: repo_unknown`），不入库 |

**示例配置：**

```dotenv
# 允许 GitHub 组织 myorg 下所有仓库，以及公司内网 Codeup 仓库
AITRACK_REPO_WHITELIST=git@github.com:myorg/,https://github.com/myorg/,git@codeup.aliyun.com:mycompany/

# 强制模式：白名单外的 repo 直接拒绝
AITRACK_REPO_ENFORCE=true
```

### 3.2 添加/删除白名单仓库

白名单通过修改环境变量并重启服务端生效。**没有热更新 API**，修改后必须重启。

**添加新仓库前缀：**

```bash
# 步骤 1：编辑 .env 文件，在 AITRACK_REPO_WHITELIST 中追加新前缀
# 例如，当前值为 git@github.com:myorg/
# 追加后为：git@github.com:myorg/,git@codeup.aliyun.com:newteam/

# 步骤 2：执行零停机滚动重启（Java 服务端，PostgreSQL 模式）
# 先确认当前服务状态
docker compose -f docker/docker-compose.yml ps

# 重新加载环境变量并重启服务端容器（不重启 DB 容器）
export $(grep -v '^#' .env | xargs)
docker compose -f docker/docker-compose.yml up -d --no-deps --force-recreate aitrack-server-java

# 步骤 3：验证服务已恢复
curl -s http://localhost:8080/actuator/health
# 期望: {"status":"UP"}
```

**零停机滚动重启注意事项：**

- `--no-deps` 参数确保不重启 ParadeDB 容器，数据不丢失
- `--force-recreate` 强制用新环境变量重建容器
- 重启过程中（通常 5-10 秒）客户端会收到连接拒绝，客户端会自动重试，不丢失数据
- 如果有反向代理（nginx），可先将流量切至备用节点再重启

**删除仓库前缀：**

直接从 `AITRACK_REPO_WHITELIST` 中移除对应前缀后执行同样的重启步骤。删除后，该 repo 的后续上报会根据 `AITRACK_REPO_ENFORCE` 的值决定是标记还是拒绝；历史数据不受影响。

### 3.3 enforce=false 与 enforce=true 的行为差异

**enforce=false（默认，推荐初期部署使用）：**

- 不在白名单内的 edit 记录**仍然入库**
- 记录被标记 `flagged: repo_unknown`，可在 `GET /edits` 查询结果中看到 `flagged: true`
- 服务端日志出现 `WARN flagged: repo_unknown` 条目
- 适合场景：刚启动白名单管理，需要先观察有哪些 repo 在上报，再决定是否加入白名单

**enforce=true（严格模式）：**

- 不在白名单内的 edit 记录**直接拒绝，不入库**
- 被拒记录出现在上传响应的 `rejected` 数组中（`reason: "repo_unknown"`）
- 客户端本地 `retry_count += 1`，超过 5 次后放弃重试
- 服务端日志出现 `WARN rejected: repo_unknown` 条目
- 适合场景：白名单已经稳定覆盖所有合法 repo，需要防止非预期 repo 的数据进入系统

> ⚠️ **注意：** 从 enforce=false 切换到 enforce=true 之前，建议先在 enforce=false 模式下运行至少一周，通过查询 `flagged: repo_unknown` 的记录确认白名单覆盖完整，避免合法开发活动被误拒。

---

## 4. 设备与心跳监控

### 4.1 查看所有设备

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool
```

**完整返回示例：**

```json
[
  {
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "hostname": "MacBook-Pro.local",
    "client_version": "1.2.0",
    "last_seen": "2026-05-19T10:00:00Z",
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
    "client_version": "1.2.0",
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

**过滤 silent 设备（快速巡检）：**

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -c "
import json, sys
devices = json.load(sys.stdin)
silent = [d for d in devices if d.get('silent')]
print(f'Total devices: {len(devices)}, Silent: {len(silent)}')
for d in silent:
    print(f\"  owner={d['owner']} hostname={d['hostname']} last_seen={d['last_seen']}\")
"
```

### 4.2 字段说明

| 字段 | 说明 |
|------|------|
| `device_id` | 客户端首次 `aitrack init` 时自动生成的 UUIDv4，同一台机器在不同时间生成的 device_id 相同 |
| `token_key` | 该设备使用的 masked token 标识（前6位 + "…" + 后4位），与 `POST /admin/tokens` 响应中的 `token_key` 对应 |
| `owner` | 签发 token 时填写的 `owner` 字段，用于识别设备归属 |
| `hostname` | 上报机器的 OS hostname（v1.1 新增），同一 `token_key` 出现多个不同 `hostname` 是正常场景（一个 credential 可在多台机器使用）|
| `client_version` | 最后一次心跳时的 aitrack 客户端版本 |
| `last_seen` | 最后一次心跳的 UTC 时间。**注意**：心跳每次 capture 结束后最多 1 小时触发一次，因此 `last_seen` 最长可落后实际活跃时间约 1 小时，这不是异常。 |
| `hooks.claude` / `hooks.codex` / `hooks.cursor` | 对应工具的钩子是否已安装（`true`=已安装，`false`=未安装或已移除） |
| `pending_count` | 客户端本地待同步的记录数。持续偏大（如 > 50）说明客户端数据积压，需排查网络或鉴权问题 |
| `silent` | `true` 表示该设备所有工具的钩子均已被移除（所有 hooks 均为 `false`），可能是主动规避采集的信号 |

**hook_active=false 代表什么：**

单个工具的钩子为 `false` 可能有以下原因：

1. 该开发者从未使用该工具（正常）
2. 钩子在 `aitrack init` 时未选择安装该工具的选项（正常）
3. 开发者手动执行了 `aitrack remove --claude`（需关注）
4. 工具更新后覆盖了钩子配置文件（偶发，开发者需重新执行 `aitrack init --claude`）
5. `~/.claude/settings.json` 中存在其他工具注册的 PostToolUse 钩子，`aitrack init --claude` 会在 stderr 输出冲突警告（v1.6.3+），但不会中止安装；若最终未写入，需开发者确认文件内容后手动处理冲突

**last_seen 延迟说明：**

心跳由客户端节流发送：每次 capture 执行结束后检查，若距上次心跳超过 1 小时才发送。因此：
- 正常活跃开发者的 `last_seen` 最多落后当前时间约 1 小时
- `last_seen` 超过 **48 小时**未更新且开发者应当在工作日活跃 → 客户端可能已离线、钩子异常、或网络中断
- `last_seen` 超过 **7 天** → 强烈建议联系开发者检查

### 4.3 钩子被静默移除的告警流程

**什么情况触发：**

- `GET /api/v1/ai-track/devices` 中出现 `"silent": true`（所有 hooks 全为 `false`）
- 某开发者的 `last_seen` 超过配置的告警阈值（建议 48 小时）

**监控脚本（建议每小时定时执行）：**

```bash
#!/bin/bash
# aitrack 设备巡检脚本
# 建议通过 cron 每小时执行：
# 0 * * * * /opt/aitrack/scripts/check_devices.sh >> /var/log/aitrack-monitor.log 2>&1

TOKEN="$AITRACK_MONITOR_TOKEN"  # 使用专用监控 token
SERVER="http://localhost:8080"
ALERT_AFTER_HOURS=48

DEVICES=$(curl -s "${SERVER}/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN")

SILENT_COUNT=$(echo "$DEVICES" | python3 -c "
import json, sys
from datetime import datetime, timezone, timedelta

devices = json.load(sys.stdin)
now = datetime.now(timezone.utc)
alert_delta = timedelta(hours=${ALERT_AFTER_HOURS})

silent = [d for d in devices if d.get('silent')]
stale = [d for d in devices
         if not d.get('silent')
         and d.get('last_seen')
         and (now - datetime.fromisoformat(d['last_seen'].replace('Z','+00:00'))) > alert_delta]

print(f'silent={len(silent)} stale={len(stale)}')
for d in silent:
    print(f\"SILENT: owner={d['owner']} hostname={d['hostname']} last_seen={d['last_seen']}\")
for d in stale:
    print(f\"STALE: owner={d['owner']} hostname={d['hostname']} last_seen={d['last_seen']}\")
")

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $SILENT_COUNT"
```

**如何通知开发者：**

1. 从告警输出中获取 `owner` 字段（对应签发时填写的 owner）
2. 通过内部 IM 或邮件通知开发者：钩子状态异常，请执行以下命令重新安装

```bash
# 通知开发者执行（根据其使用的工具选择参数）
aitrack status                    # 先确认当前状态
aitrack init --claude \           # 重新安装 Claude Code 钩子（指定单个工具）
  --api-url https://aitrack.company.internal \
  --credential <原有credential>   # credential 不变，无需重新签发

# 或使用自动探测模式（v1.6.3+），自动检测 ~/.claude/~/.codex/~/.cursor 并安装所有已找到的工具钩子
aitrack init \
  --api-url https://aitrack.company.internal \
  --credential <原有credential>
```

3. 若开发者已离职或设备废弃，在下次密钥轮换时自然清除（无专用删除 API）

---

## 5. 效能数据查询

`GET /api/v1/ai-track/stats` 端点支持四种聚合维度。所有查询只需 Bearer token，不需要 HMAC 签名头。

### 5.1 四种维度查询示例

**维度 1：按 token（开发者）汇总**

最常用，用于效能月报、按人统计 AI 编码活跃度。

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"

curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool
```

典型返回：

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

**维度 2：按 repo（仓库）汇总**

用于了解哪个项目 AI 编辑最活跃，评估各项目对 AI 工具的依赖程度。

```bash
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=repo" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool
```

典型返回：

```json
[
  {
    "repo_url": "git@github.com:myorg/backend.git",
    "edit_count": 198,
    "added_lines": 5200,
    "removed_lines": 1870
  },
  {
    "repo_url": "git@github.com:myorg/frontend.git",
    "edit_count": 31,
    "added_lines": 920,
    "removed_lines": 260
  }
]
```

**维度 3：按 device（设备 UUID）汇总**

用于识别同一开发者多台机器的活动分布，确认数据是否有合理的设备多样性。

```bash
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=device" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool
```

典型返回：

```json
[
  {
    "device_id": "550e8400-e29b-41d4-a716-446655440000",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 98,
    "added_lines": 2700,
    "removed_lines": 830
  },
  {
    "device_id": "aa1b2c3d-e4f5-6789-abcd-ef0123456789",
    "token_key": "abcdef…7890",
    "owner": "alice",
    "edit_count": 44,
    "added_lines": 1120,
    "removed_lines": 410
  }
]
```

**维度 4：按 hostname（机器名）汇总**

用于人工排查多设备共用同一 token 的情况，验证 hostname 与已知开发者机器名是否一致。

```bash
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=hostname" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool
```

典型返回：

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

> 同一 `token_key` 出现多个 `hostname` 是正常情况（CONTRACT.md v1.2 明确支持一个 credential 用于多台机器）。若某个 hostname 数据量异常偏高（如单台机器贡献 90%+ 的记录），结合 `/devices` 端点查看该设备的钩子状态和 pending_count 进行进一步排查。

### 5.2 常见查询场景

**场景 1：查某人本周数据**

当前 `GET /stats` 不支持时间范围过滤。需要通过 `GET /edits` 分页接口结合 token_key 过滤，或直接查询数据库获取有时间条件的聚合数据。

```bash
# 通过 /edits 接口获取指定 token 最近记录（用于粗略估计）
TOKEN="aitrack_abcdef1234567890abcdef1234567890"
ALICE_TOKEN_KEY="abcdef%E2%80%A67890"  # URL 编码的 masked token key

curl -s "http://localhost:8080/api/v1/ai-track/edits?token_key=${ALICE_TOKEN_KEY}&page=0&size=100" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -c "
import json, sys
from datetime import datetime, timezone, timedelta
data = json.load(sys.stdin)
week_ago = datetime.now(timezone.utc) - timedelta(days=7)
week_items = [i for i in data['items']
              if datetime.fromisoformat(i['timestamp'].replace('Z','+00:00')) > week_ago]
added = sum(i['added_lines'] for i in week_items)
removed = sum(i['removed_lines'] for i in week_items)
print(f'本周编辑次数: {len(week_items)}, 新增行: {added}, 删除行: {removed}')
"
```

**直接查数据库（精确时间范围）：**

```bash
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT
      t.owner,
      COUNT(*) AS edit_count,
      SUM(e.added_lines) AS added_lines,
      SUM(e.removed_lines) AS removed_lines
    FROM edit_records e
    JOIN tokens t ON t.token_hash = encode(sha256(e.token_key::bytea), 'hex')
    WHERE e.created_at >= NOW() - INTERVAL '7 days'
    GROUP BY t.owner
    ORDER BY edit_count DESC;
  "
```

**场景 2：查某 repo 所有设备的活动**

```bash
# 先通过 stats?group_by=repo 确认 repo_url 精确值
# 再通过 /edits 过滤该 repo
curl -s "http://localhost:8080/api/v1/ai-track/edits?repo=myorg%2Fbackend&page=0&size=50" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -c "
import json, sys
data = json.load(sys.stdin)
print(f'Total records: {data[\"total\"]}')
devices = {}
for item in data['items']:
    key = item.get('token_key', 'unknown')
    devices[key] = devices.get(key, 0) + 1
for k, v in sorted(devices.items(), key=lambda x: -x[1]):
    print(f'  {k}: {v} edits')
"
```

**场景 3：对比工具使用量（claude vs codex vs cursor）**

```bash
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT
      tool,
      COUNT(*) AS edit_count,
      SUM(added_lines) AS total_added,
      SUM(removed_lines) AS total_removed
    FROM edit_records
    WHERE created_at >= NOW() - INTERVAL '30 days'
    GROUP BY tool
    ORDER BY edit_count DESC;
  "
```

---

## 6. 语义检索（ParadeDB 模式）

### 6.1 前提条件

语义检索端点（BM25 全文检索和 ANN 向量检索）**仅在以下条件同时满足时可用**：

1. Java 服务端：`SPRING_PROFILES_ACTIVE` 包含 `postgres`
2. 或 Go 服务端：`DATABASE_URL` 已设置且指向 PostgreSQL/ParadeDB
3. ParadeDB 容器正在运行且已通过健康检查
4. BM25 索引和 HNSW 索引已在首次部署时创建（见 `OPERATIONS.md` 第 3 节步骤 7）

**验证当前模式是否支持语义检索：**

```bash
# BM25 检索探测（有效 Admin Key，空查询）
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/api/v1/ai-track/edits/search?q=test" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY"
# 返回 200 或 400（参数错误）= ParadeDB 模式正常
# 返回 501 = 当前为 H2/SQLite 模式，语义检索不可用
```

### 6.2 BM25 全文检索

基于 ParadeDB `pg_search` 插件，对 `diff_hunk` 和 `prompt_summary` 字段进行 BM25 全文检索，结果按相关性分数降序返回。

**鉴权**：`X-Admin-Key`（不是 Bearer token）

```bash
# 搜索包含 "refactor authentication" 的编辑记录
curl -s "http://localhost:8080/api/v1/ai-track/edits/search?q=refactor+authentication" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  | python3 -m json.tool
```

**带过滤条件的检索：**

```bash
# 只搜索指定开发者（token_key 过滤）的记录
curl -s "http://localhost:8080/api/v1/ai-track/edits/search?q=database+migration&token_key=abcdef%E2%80%A67890&limit=10" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY"

# 只搜索指定仓库的记录
curl -s "http://localhost:8080/api/v1/ai-track/edits/search?q=auth+handler&repo=myorg%2Fbackend&limit=20" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY"
```

**查询参数：**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `q` | string | 是 | — | 检索文本（支持多词，空格分隔）|
| `limit` | int | 否 | 20 | 最大结果数（上限 100）|
| `token_key` | string | 否 | — | 按开发者过滤（masked 格式）|
| `repo` | string | 否 | — | 按仓库过滤（URL 部分匹配）|

**返回示例：**

```json
{
  "query": "refactor authentication",
  "total": 3,
  "hits": [
    {
      "record_id": "abc123",
      "token_key": "abcdef…7890",
      "repo": "myorg/backend",
      "file_path": "src/auth/handler.go",
      "diff_hunk": "@@ -10,5 +10,8 @@ func HandleLogin(...",
      "ai_lines_added": 12,
      "ai_lines_removed": 3,
      "ts": 1748000000,
      "score": 0.8734
    }
  ]
}
```

`score` 为 BM25 相关性分数，越高越相关。结果按分数降序排列。

**返回字段说明：**

| 字段 | 说明 |
|------|------|
| `query` | 原始查询词（回显）|
| `total` | 匹配记录总数 |
| `hits` | 命中记录列表 |
| `hits[].record_id` | 记录 ID，可用于直接查询 |
| `hits[].score` | BM25 相关性分数，越高越相关 |
| `hits[].ts` | 编辑时间（Unix 秒）|

**错误码：**

| 状态码 | 原因 |
|--------|------|
| 400 | `q` 参数缺失或为空 |
| 403 | `X-Admin-Key` 缺失或无效 |
| 501 | 服务端未使用 PostgreSQL/ParadeDB 模式（见下方说明）|

### 6.3 ANN 向量相似检索

基于 pgvector HNSW 索引，通过余弦距离查找语义相似的编辑记录。**需要记录已包含 embedding 向量**（由外部嵌入服务填充，当前为 P2 待开发功能，详见第 8.3 节）。

**鉴权**：`X-Admin-Key`

```bash
# 用 384 维向量检索语义相似的编辑记录
# 注意：embedding 数组必须是 384 维（all-MiniLM-L6-v2 模型输出）
curl -s -X POST "http://localhost:8080/api/v1/ai-track/edits/similar" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "embedding": [0.023, -0.147, 0.891, 0.034, -0.221, "...（共 384 个浮点数）"],
    "limit": 10,
    "token_key": "abcdef…7890",
    "repo": "myorg/backend"
  }'
```

**请求体字段：**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `embedding` | float[384] | 是 | 查询向量，必须是 384 维（all-MiniLM-L6-v2 模型）|
| `limit` | int | 否 | 最大结果数，默认 10，上限 50 |
| `token_key` | string | 否 | 按开发者过滤 |
| `repo` | string | 否 | 按仓库过滤 |

**返回示例：**

```json
{
  "hits": [
    {
      "record_id": "def456",
      "token_key": "abcdef…7890",
      "repo": "myorg/backend",
      "file_path": "src/auth/middleware.go",
      "diff_hunk": "@@ -5,3 +5,9 @@ ...",
      "ai_lines_added": 8,
      "ai_lines_removed": 1,
      "ts": 1748000100,
      "distance": 0.142
    }
  ]
}
```

`distance` 为余弦距离，范围 [0, 2]，越小越相似（0 = 完全相同，2 = 完全相反）。

**错误码：**

| 状态码 | 原因 |
|--------|------|
| 400 | `embedding` 缺失、维度不是 384，或 `limit` > 50 |
| 403 | `X-Admin-Key` 缺失或无效 |
| 501 | 非 ParadeDB 模式，或数据库中尚无 embedding 数据 |

### 6.4 非 ParadeDB 模式下的行为

> ⚠️ **注意：** 在 H2 模式（Java 默认）下，`GET /edits/search` 和 `POST /edits/similar` 返回 **`501 Not Implemented`**，这不是故障，是预期行为。Go 服务端为 PostgreSQL-only，无 H2/SQLite 模式。服务端其他功能（编辑上报、心跳、stats、devices）完全正常。

如果需要启用语义检索，参考以下操作：

```bash
# Java 服务端切换到 postgres 模式（需要 ParadeDB 运行）
# 1. 确认 ParadeDB 已启动并健康
docker compose -f docker/docker-compose.yml ps db

# 2. 重启 Java 服务端并注入 postgres profile
export $(grep -v '^#' .env | xargs)
docker compose -f docker/docker-compose.yml up -d --no-deps --force-recreate \
  -e SPRING_PROFILES_ACTIVE=postgres \
  aitrack-server-java

# 3. 确认语义检索已启用（应返回 200 或 400，不是 501）
curl -s -o /dev/null -w "%{http_code}" \
  "http://localhost:8080/api/v1/ai-track/edits/search?q=test" \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY"
```

---

## 7. 异常排查

### 7.1 大量 sig_mismatch 日志

**日志特征：**

```
WARN  rejected: sig_mismatch device_id=550e8400 token_key=abcdef…7890
```

**可能原因：**

| 原因 | 判断方式 | 处置 |
|------|----------|------|
| 客户端时钟偏差超过 `AITRACK_TIMESTAMP_WINDOW`（默认 300 秒）| 查看 `X-AiTrack-Timestamp` 与服务端时间差，或检查开发者机器时间 | 通知开发者同步 NTP：`timedatectl set-ntp true`（Linux）或系统时间设置（macOS） |
| 客户端版本与服务端协议不兼容（v1.0.x 客户端对接 v1.2.x 服务端）| 从设备列表中查 `client_version`，确认是否为 v1.2.0+ | 通知开发者升级 aitrack 客户端至 v1.2.0+，重新执行 `aitrack init --credential <新credential>` |
| 本地 SQLite 记录被第三方工具篡改 | 检查签名失败记录的 `record_id`，与正常记录做字段对比 | 如确认为篡改，记录并上报安全事件；若为误操作，通知开发者清空本地记录（`aitrack clean --all --force`）|
| hmac_secret 提取错误（极罕见，通常因 credential 格式异常）| 让开发者重新获取 credential 并重新 init | 重新签发 credential |

**处置流程：**

```bash
# 1. 统计 sig_mismatch 频率（过去 1 小时）
docker logs aitrack-server-java 2>&1 \
  | grep "sig_mismatch" \
  | awk '{print $1, $2}' \
  | sort | uniq -c | sort -rn | head -20

# 2. 提取出问题的 device_id 和 token_key
docker logs aitrack-server-java 2>&1 \
  | grep "sig_mismatch" \
  | grep -oP 'device_id=\S+|token_key=\S+'

# 3. 查询该设备的信息
TOKEN="aitrack_abcdef1234567890abcdef1234567890"
curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -c "
import json, sys
devices = json.load(sys.stdin)
for d in devices:
    if d.get('token_key') == 'abcdef…7890':
        print(json.dumps(d, indent=2))
"
```

**判断标准：**
- 单台设备偶发 1-2 次 sig_mismatch → 时钟抖动，通常可忽略
- 单台设备持续高频 sig_mismatch（> 10 次/小时）→ 版本不兼容或数据被篡改，需介入
- 多台设备同时出现 sig_mismatch → 服务端时钟问题或 `AITRACK_SECRET_KEY` 意外变更，需立即排查

### 7.2 rate_limited 频繁

**日志特征：**

```
WARN  rate_limited token_key=abcdef…7890 file_path=src/generated/schema.ts count=87
```

**可能原因：**

| 原因 | 说明 | 处置 |
|------|------|------|
| 某 (token, file_path) 组合在短时间内触发大量 AI 编辑 | 对同一文件频繁应用 AI 补丁，或有工具在循环调用 | 正常开发行为则调大 `AITRACK_RATE_LIMIT` 上限；如是工具 bug 则联系开发者排查工具配置 |
| CI/CD 管道使用开发者 token 进行自动化操作 | CI pipeline 批量处理文件会快速达到限流 | 为 CI 单独签发 token（`owner: ci-bot`），并适当调高限流上限 |
| 异常脚本或爬虫使用 token 批量上报 | 非正常用途的 token 使用 | 立即调查该 token 的完整上报记录；必要时通过轮换 `AITRACK_SECRET_KEY` 使该 token 失效 |

**调整限流上限：**

```bash
# 修改 .env 文件中的 AITRACK_RATE_LIMIT
# 例如，将默认 60 次/分钟改为 120 次/分钟
# AITRACK_RATE_LIMIT=120

# 重启服务端使新限制生效
export $(grep -v '^#' .env | xargs)
docker compose -f docker/docker-compose.yml up -d --no-deps --force-recreate aitrack-server-java
```

> ⚠️ **注意：** 调整限流时需权衡：过低会影响正常开发者，过高会使防刷量保护失效。建议先统计正常开发者的每分钟实际上报频率（通过数据库查询），再设置合理上限。

### 7.3 flagged: diff_inconsistent

**日志特征：**

```
WARN  flagged: diff_inconsistent record_id=xxx added_lines=100 diff_actual_added=3
```

**含义：** 记录中 `added_lines` 或 `removed_lines` 字段的值与 `diff_hunk` 实际内容不一致。服务端在 10 步校验链的第 5 步（diff 自洽检查）进行此校验。该记录**仍然入库**，但被标记为可疑。

**常见触发原因：**

1. 客户端版本低于 v1.2.0，使用了朴素行数统计而非 Myers/LCS diff（存在行数虚报）
2. `diff_hunk` 字段为 null 但 `added_lines > 0`（可能是适配器解析问题）
3. 文件编码导致行数计算差异（罕见）

**是否需要人工介入：**

- **偶发（< 1%）**：通常是客户端 bug，记录已入库，数据仍有参考价值，无需介入
- **某开发者持续高频触发（> 5%）**：该开发者的客户端可能版本过旧，通知其升级
- **批量多人同时触发**：可能是服务端 diff 校验逻辑变更导致，检查最近是否有服务端升级

```bash
# 查询所有 diff_inconsistent 标记记录的统计
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT
      token_key,
      COUNT(*) AS flagged_count,
      MIN(created_at) AS first_seen,
      MAX(created_at) AS last_seen
    FROM edit_records
    WHERE flag_reason = 'diff_inconsistent'
    GROUP BY token_key
    ORDER BY flagged_count DESC;
  "
```

### 7.4 客户端 `aitrack status` 报连接失败

**症状：** 开发者执行 `aitrack status` 或 `aitrack heartbeat` 时报错，如 `connection refused`、`401 Unauthorized`、`timeout`。

**排查链：**

**步骤 1：确认服务端是否在线**

```bash
# 管理员在服务端宿主机上执行
curl -s http://localhost:8080/actuator/health
# 期望: {"status":"UP"}

# 如果服务端不在线，查看容器状态
docker compose -f docker/docker-compose.yml ps
docker logs aitrack-server-java --tail 50
```

**步骤 2：确认开发者机器到服务端的网络连通性**

```bash
# 在开发者机器上执行（替换为实际服务端地址）
curl -v -s -o /dev/null https://aitrack.company.internal/actuator/health
# 或
telnet aitrack.company.internal 443

# 如果连接超时，检查：
# - VPN 是否已连接
# - DNS 是否能解析服务端域名
# - 防火墙规则
```

**步骤 3：确认 credential 是否有效**

```bash
# 在开发者机器上执行
# 从 config.toml 读取 credential 并手动测试
CREDENTIAL=$(awk -F'"' '/credential/{print $2}' ~/.aitrack/config.toml)
TOKEN="${CREDENTIAL%%-*}"

curl -s -o /dev/null -w "%{http_code}" \
  "https://aitrack.company.internal/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"
# 200 = token 有效
# 401 = token 无效（credential 可能已失效，需重新签发）
```

**步骤 4：确认客户端版本与服务端协议兼容**

```bash
# 查看客户端版本
aitrack --version
# 需要 v1.2.0+ 才支持 credential 格式

# 查看服务端版本（从日志或 actuator 获取）
curl -s http://localhost:8080/actuator/info | python3 -m json.tool
```

**步骤 5：检查时钟偏差**

```bash
# 查看开发者机器与 NTP 服务器的时间差
# macOS
date && sntp -s time.apple.com

# Linux
timedatectl status
# 确认 "System clock synchronized: yes"
```

**步骤 6：查看服务端日志中该设备的记录**

```bash
# 用开发者的 device_id 过滤日志
DEVICE_ID="550e8400-e29b-41d4-a716-446655440000"
docker logs aitrack-server-java 2>&1 \
  | grep "$DEVICE_ID" \
  | tail -20
```

---

## 8. 数据保留与清理

### 8.1 当前数据保留策略

> ⚠️ **注意：** aitrack 当前**没有自动 TTL 或数据过期机制**。数据会无限积累直到手动清理或磁盘满。建议监控磁盘使用率，设置 > 80% 时告警（见 `OPERATIONS.md` 第 5.3 节）。

**查看当前数据量：**

```bash
# 查看数据库总大小
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT pg_size_pretty(pg_database_size('aitrack')) AS db_size;
  "

# 查看各表行数和大小
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT
      relname AS table_name,
      n_live_tup AS row_count,
      pg_size_pretty(pg_total_relation_size(relid)) AS total_size
    FROM pg_stat_user_tables
    ORDER BY pg_total_relation_size(relid) DESC;
  "

# 查看最早和最新记录时间
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT
      MIN(created_at) AS oldest_record,
      MAX(created_at) AS newest_record,
      COUNT(*) AS total_records
    FROM edit_records;
  "
```

### 8.2 手动清理步骤

**清理前必须完成的备份：**

```bash
# 步骤 1：逻辑备份（pg_dump，不需要停服）
mkdir -p /opt/aitrack/backups
docker compose -f docker/docker-compose.yml exec db \
  pg_dump -U aitrack -d aitrack -Fc \
  > /opt/aitrack/backups/aitrack-pre-cleanup-$(date +%Y%m%d-%H%M%S).dump

# 确认备份文件已生成且大小合理
ls -lh /opt/aitrack/backups/

# 步骤 2：验证备份可恢复（可选，建议在测试环境验证）
# docker compose exec db pg_restore --list < backup.dump | head -20
```

**清理 N 天前的旧数据：**

```bash
# 先用 SELECT 确认要删除的数据量
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT COUNT(*) AS will_delete
    FROM edit_records
    WHERE created_at < NOW() - INTERVAL '180 days';
  "

# 确认无误后执行删除（示例：清理 180 天前的数据）
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    DELETE FROM edit_records
    WHERE created_at < NOW() - INTERVAL '180 days';
  "

# 清理心跳历史（设备历史数据，可保留更短时间）
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    DELETE FROM device_heartbeats
    WHERE last_seen < NOW() - INTERVAL '90 days';
  "

# 清理后执行 VACUUM 释放磁盘空间
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "VACUUM ANALYZE edit_records;"
```

**定时清理脚本（cron 示例，每周日凌晨 3 点执行）：**

```bash
# 添加到 crontab：crontab -e
# 保留 180 天数据，每周清理一次
0 3 * * 0 cd /opt/aitrack && \
  docker compose -f docker/docker-compose.yml exec -T db \
  psql -U aitrack -d aitrack -c \
  "DELETE FROM edit_records WHERE created_at < NOW() - INTERVAL '180 days'; VACUUM ANALYZE edit_records;" \
  >> /var/log/aitrack-cleanup.log 2>&1
```

> ⚠️ **注意：** 删除操作不可撤销。执行前务必确认备份完整，且备份文件已转移到独立存储（不在同一磁盘）。建议在低峰时段（如凌晨）执行大批量删除，避免影响服务性能。

### 8.3 Embedding 回填脚本（预留）

> **说明：** Embedding 回填功能目前为 P2 待开发状态（路线图 v1.4.0）。当前 `edit_records.embedding` 列存在但为 NULL，ANN 向量检索端点在无 embedding 数据时返回 501。

**预留占位说明：**

- 目标：为已入库的历史编辑记录批量生成语义嵌入向量（all-MiniLM-L6-v2，384 维）
- 实现方案：独立 Python 脚本，批量读取 `diff_hunk` → 调用嵌入 API → 写入 `embedding` 列
- 预计开发周期：P2，详见 `plans/roadmap.md`
- 当前手动触发方式：等待 roadmap 实现，暂无回填途径

脚本上线后，操作说明将补充至本节。

---

## 9. 安全操作清单

### 9.1 定期轮换 AITRACK_ADMIN_KEY

**建议轮换周期：** 每 90 天，或发生管理员人员变动时。

**影响范围：** 仅影响 `POST /admin/tokens` 等 `/admin/**` 接口的调用者（通常只有系统管理员），不影响已签发的开发者 credential 和正在进行的数据上报。

```bash
# 步骤 1：生成新的 AITRACK_ADMIN_KEY
NEW_ADMIN_KEY=$(openssl rand -hex 32)
echo "新 AITRACK_ADMIN_KEY: $NEW_ADMIN_KEY"
# 立即存入密码管理工具

# 步骤 2：更新 .env 文件
# 将 AITRACK_ADMIN_KEY 的值替换为 NEW_ADMIN_KEY

# 步骤 3：重启服务端使新密钥生效（不影响 DB）
export $(grep -v '^#' .env | xargs)
docker compose -f docker/docker-compose.yml up -d --no-deps --force-recreate aitrack-server-java

# 步骤 4：验证新密钥有效
curl -s -o /dev/null -w "%{http_code}" \
  -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $NEW_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"owner":"key-rotation-test","note":"verify-new-key"}'
# 期望返回: 200

# 步骤 5：验证旧密钥已失效
curl -s -o /dev/null -w "%{http_code}" \
  -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: <旧密钥>" \
  -H "Content-Type: application/json" \
  -d '{"owner":"test"}'
# 期望返回: 401

# 步骤 6：通知所有管理员更新本地 AITRACK_ADMIN_KEY 配置
```

### 9.2 轮换 AITRACK_SECRET_KEY

> ⚠️ **注意：** 这是破坏性操作。`AITRACK_SECRET_KEY` 用于加密存储每个 credential 中的 `hmac_secret`。轮换后，服务端无法解密旧密钥加密的 `hmac_secret`，**所有已签发的 credential 立即失效**，所有开发者的 aitrack 客户端将无法上报数据，直到重新签发 credential。

**必须提前完成的准备工作：**

1. 通知所有开发者即将进行密钥轮换，预计影响时间
2. 确认已知所有活跃 token 的 `owner`（通过 `GET /stats?group_by=token` 获取完整列表）
3. 准备好为所有人重新签发 credential 的操作脚本

**执行步骤：**

```bash
# 步骤 1：获取当前所有活跃 token 的 owner 列表
TOKEN="aitrack_abcdef1234567890abcdef1234567890"
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -c "
import json, sys
stats = json.load(sys.stdin)
print('需要重新签发的 owner 列表：')
for s in stats:
    print(f\"  owner={s['owner']} token_key={s['token_key']} edit_count={s['edit_count']}\")
"

# 步骤 2：备份数据库（重要：密钥轮换前必须备份）
docker compose -f docker/docker-compose.yml exec db \
  pg_dump -U aitrack -d aitrack -Fc \
  > /opt/aitrack/backups/pre-key-rotation-$(date +%Y%m%d-%H%M%S).dump

# 步骤 3：生成新的 AITRACK_SECRET_KEY
NEW_SECRET_KEY=$(openssl rand -base64 32)
echo "新 AITRACK_SECRET_KEY: $NEW_SECRET_KEY"
# 立即存入密码管理工具

# 步骤 4：更新 .env 并重启服务端
# 修改 .env 中的 AITRACK_SECRET_KEY 为新值
export $(grep -v '^#' .env | xargs)
docker compose -f docker/docker-compose.yml up -d --no-deps --force-recreate aitrack-server-java

# 步骤 5：为所有开发者重新签发 credential
for owner in alice bob charlie ci-bot; do
  echo "=== 重新签发 $owner ==="
  curl -s -X POST http://localhost:8080/admin/tokens \
    -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"owner\":\"${owner}\",\"note\":\"key-rotation-$(date +%Y%m%d)\"}"
  echo ""
done

# 步骤 6：逐一通知开发者更新 credential
# 开发者执行：
# aitrack init --credential <新credential> --api-url <url>
```

### 9.3 生产环境禁止 H2 Console

H2 Console 是 Spring Boot 内置的数据库管理界面，只在 `dev` profile 下启用。

**确认当前状态：**

```bash
# 检查当前 profile
docker inspect aitrack-server-java \
  | python3 -c "
import json, sys
data = json.load(sys.stdin)
env = data[0].get('Config', {}).get('Env', [])
for e in env:
    if 'SPRING_PROFILES_ACTIVE' in e:
        print(e)
"

# 尝试访问 H2 Console（生产环境应该返回 404）
curl -s -o /dev/null -w "%{http_code}" \
  http://localhost:8080/h2-console
# 生产环境期望: 404
# 如果返回 200，说明 dev profile 被意外激活，需要立即修复
```

**规则：**

- `SPRING_PROFILES_ACTIVE=postgres`：H2 Console **自动禁用**（postgres profile 不包含 dev）
- `SPRING_PROFILES_ACTIVE=default`（H2 模式，仅评估用）：H2 Console 同样禁用（default 不是 dev）
- `SPRING_PROFILES_ACTIVE=dev`（仅本地开发）：H2 Console 启用，**禁止在生产环境使用**

生产环境启动命令中**不应包含** `dev` profile：

```bash
# 正确（生产）
SPRING_PROFILES_ACTIVE=postgres

# 错误（禁止在生产使用）
SPRING_PROFILES_ACTIVE=dev,postgres  # 会意外启用 H2 console
```

### 9.4 安全操作快速清单

定期（建议每季度）逐项确认：

- [ ] `AITRACK_ADMIN_KEY` 已在密码管理工具中记录，且距上次轮换不超过 90 天
- [ ] `AITRACK_SECRET_KEY` 已在密码管理工具中记录，生产环境通过环境变量注入（不在代码或 Git 中）
- [ ] `.env` 文件未提交到 Git（确认 `.gitignore` 包含 `.env`）
- [ ] 生产服务端 `SPRING_PROFILES_ACTIVE` 不包含 `dev`，H2 Console 不可访问（`/h2-console` 返回 404）
- [ ] `/admin/**` 接口通过反向代理 ACL 限制访问（仅允许运维 IP）
- [ ] ParadeDB 5432 端口未对公网暴露（`docker ps` 确认端口绑定为 `127.0.0.1:5432` 或内网 IP）
- [ ] `GET /devices` 中 `silent=true` 的设备已跟进处理或确认无害
- [ ] 服务端日志中 `sig_mismatch` 频率在正常范围内（< 1% 的上报请求）
- [ ] 磁盘使用率低于 80%（`df -h` 确认 pgdata 卷所在分区）
- [ ] 客户端 `~/.aitrack/config.toml` 和 `~/.aitrack/records.db` 权限为 0600（通知开发者自检）

---

*文档版本：v1.2.0-admin-guide | 最后更新：2026-05-19*
