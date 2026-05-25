# aitrack — 生产运维手册

> **适用版本**：v1.3.0+  
> **文件性质**：内部文档，仅限 Codeup 私有仓库，禁止发布至 GitHub

---

## 1. 部署架构

### 1.1 推荐拓扑

| 场景 | 拓扑 | 说明 |
|------|------|------|
| 小团队（≤ 50 人） | 单机 Docker Compose | 服务端与 ParadeDB 部署于同一宿主机，运维成本最低 |
| 中大团队（> 50 人） | 分离部署 | 服务端容器与数据库分机部署，可独立扩缩容 |

### 1.2 组件依赖关系

```
  开发者机器                     服务端宿主机
  ─────────                     ──────────────────────────────────────
  AI 工具钩子                         ┌──────────────┐
  (Claude/Codex/Cursor)               │  反向代理     │  ← 公司内网 443
        │ stdin JSON                  │ (nginx/Caddy) │
        ▼                             └──────┬───────┘
  aitrack (Rust CLI)                         │ HTTP
  ├─ ~/.aitrack/config.toml                  ▼
  └─ ~/.aitrack/records.db         ┌──────────────────────────────────┐
        │                          │  Java 服务端 :8080               │
        │  POST /edits             │  (aitrack-server-java)           │
        │  POST /heartbeat    ───► │  Spring Boot 3 · H2 / ParadeDB   │
        │  Bearer token            │                                  │
        │                          │  — 或 —                          │
        │                          │  Go 服务端 :8081                  │
        │                          │  (aitrack-server-go)             │
        │                          │  chi v5 · ParadeDB               │
        │                          └──────────────┬───────────────────┘
        │                                         │ JDBC / pgx
        │                                         ▼
        │                          ┌──────────────────────────────────┐
        │                          │  ParadeDB (PostgreSQL 兼容)       │
        │                          │  pg_search (BM25) + pgvector      │
        │                          │  pgdata volume                    │
        │                          └──────────────────────────────────┘

  管理员终端
        │  X-Admin-Key
        └────────────────────────► POST /admin/tokens
                                   GET  /api/v1/ai-track/stats
                                   GET  /api/v1/ai-track/devices
```

同一宿主机上建议只运行 Java 服务端**或** Go 服务端，两者不同时部署（功能等价，按团队技术栈选择一个即可）。

### 1.3 网络端口规划

| 端口 | 服务 | 协议 | 对外暴露建议 |
|------|------|------|-------------|
| 8080 | Java 服务端 | HTTP | 不直接对公网暴露，通过反向代理转发 |
| 8081 | Go 服务端 | HTTP | 不直接对公网暴露，通过反向代理转发 |
| 5432 | ParadeDB (PostgreSQL) | TCP | 仅宿主机内部或私有网段访问，禁止公网 |
| 443 | nginx / Caddy（反向代理） | HTTPS | 对内网或 VPN 暴露 |

---

## 2. 环境要求

### 2.1 硬件最低规格（按团队规模）

| 规模 | CPU | 内存 | 磁盘（含数据） | 估算编辑事件量/天 |
|------|-----|------|--------------|----------------|
| ≤ 10 人 | 1 vCPU | 512 MB | 10 GB | 500–2000 条 |
| ≤ 50 人 | 2 vCPU | 2 GB | 50 GB | 2500–10000 条 |
| ≤ 200 人 | 4 vCPU | 4 GB（+独立 PG 节点） | 200 GB | 10000–40000 条 |

> 估算基准：每位开发者每天约 50–200 次编辑事件，每条记录约 2–5 KB（含 diff_hunk）。ParadeDB 的向量索引（HNSW）对内存压力较大，embedding 列大量填充后建议为数据库节点单独分配 ≥ 2 GB 内存。

### 2.2 软件依赖版本

| 依赖 | 最低版本 | 说明 |
|------|----------|------|
| Docker | 20.10+ | 宿主机容器运行时 |
| docker compose（插件） | 2.x | 推荐用 `docker compose`（V2），不推荐旧版 `docker-compose` 命令 |
| ParadeDB | paradedb/paradedb:latest | PostgreSQL 兼容，内置 pg_search + pgvector，生产推荐（v1.3.0+ 必须） |
| PostgreSQL（替代方案） | 16+ | 若不使用 ParadeDB 语义检索，可改用标准 PG 16；BM25/ANN 端点不可用 |
| JDK | 17（容器内已包含） | Java 服务端运行时，使用镜像时无需宿主机安装 |
| Go | 1.25（容器内已包含） | Go 服务端编译运行时，使用镜像时无需宿主机安装 |
| openssl | 任意版本 | 宿主机生成随机密钥用 |

### 2.3 操作系统兼容性

| 操作系统 | 支持状态 | 备注 |
|----------|----------|------|
| Linux (x86_64) | 推荐 | 生产首选，CI/CD 经过完整验证 |
| Linux (aarch64) | 支持 | ARM 服务器或苹果 M 系列宿主机可用 |
| macOS (Apple Silicon) | 开发/评估用 | 可运行全套 Docker Compose，不推荐用于生产 |
| Windows | 不支持 | Docker Desktop 未经测试 |

---

## 3. 首次部署步骤

> 以下步骤以 Java 服务端 + ParadeDB（生产推荐）为例。如使用 Go 服务端，步骤 5 中的启动命令不同，其余相同。

### 步骤 1：克隆仓库

```bash
git clone <Codeup-内网-repo-url> aitrack
cd aitrack
```

### 步骤 2：生成密钥

```bash
# AITRACK_ADMIN_KEY：管理接口鉴权，32 字节随机十六进制（64 个字符）
export AITRACK_ADMIN_KEY=$(openssl rand -hex 32)

# AITRACK_SECRET_KEY：AES-256-GCM 加密存储 hmac_secret，base64 编码的 32 字节
export AITRACK_SECRET_KEY=$(openssl rand -base64 32)

echo "AITRACK_ADMIN_KEY=$AITRACK_ADMIN_KEY"
echo "AITRACK_SECRET_KEY=$AITRACK_SECRET_KEY"
# 将以上两行输出值安全记录到密码管理工具，后续步骤需要用到
```

**重要**：两个密钥均仅显示一次。`AITRACK_ADMIN_KEY` 用于签发 credential；`AITRACK_SECRET_KEY` 用于加密存储 hmac_secret，丢失后无法解密已有数据。

### 步骤 3：配置 .env 文件

在仓库根目录创建 `.env`（已在 `.gitignore` 中排除，不会被提交）：

```dotenv
# ─── 必填：安全密钥 ────────────────────────────────────────────────
AITRACK_ADMIN_KEY=<步骤2生成的64位十六进制>
AITRACK_SECRET_KEY=<步骤2生成的base64字符串>

# ─── 必填：ParadeDB 数据库密码 ────────────────────────────────────
AITRACK_DB_PASSWORD=<自定义强密码，生产环境不使用默认值aitrack_secret>

# ─── 可选：业务参数（保持默认值即可）────────────────────────────────
# 请求时间戳允许偏差（秒），超出返回 401
AITRACK_TIMESTAMP_WINDOW=300

# 每（token, file_path）每小时最多接受的编辑数
AITRACK_RATE_LIMIT_PER_HOUR=30

# 单条记录 added_lines 上限，超出标记为 flagged: oversized
AITRACK_MAX_ADDED_LINES=5000

# 是否强制拒绝不在白名单内的 repo_url（true=强制拒绝，false=仅标记）
AITRACK_REPO_WHITELIST_ENFORCE=false

# repo_url 白名单，逗号分隔（留空表示不限制）
# AITRACK_REPO_WHITELIST_URLS=git@github.com:myorg/,https://github.com/myorg/

# ─── 可选：Go 服务端 DATABASE_URL（选用 Go 服务端时填写）───────────
# DATABASE_URL=postgres://aitrack:<AITRACK_DB_PASSWORD>@localhost:5432/aitrack

# ─── 可选：Java 服务端 ParadeDB 连接参数 ─────────────────────────
AITRACK_DB_HOST=localhost
AITRACK_DB_PORT=5432
AITRACK_DB_NAME=aitrack
AITRACK_DB_USER=aitrack
```

### 步骤 4：启动 ParadeDB 并等待健康检查

```bash
# 加载 .env 变量
export $(grep -v '^#' .env | xargs)

# 仅启动数据库服务，等待就绪
docker compose -f docker/docker-compose.yml up db -d

# 等待健康检查通过（最多 60 秒）
echo "等待 ParadeDB 健康检查..."
for i in $(seq 1 12); do
  if docker compose -f docker/docker-compose.yml exec db pg_isready -U aitrack -d aitrack > /dev/null 2>&1; then
    echo "ParadeDB 已就绪"
    break
  fi
  echo "  第 $i 次检查，等待 5 秒..."
  sleep 5
done
```

### 步骤 5：启动服务端

**Java 服务端（生产推荐）：**

```bash
docker compose -f docker/docker-compose.yml --profile java up -d \
  -e SPRING_PROFILES_ACTIVE=postgres \
  -e AITRACK_DB_HOST=db \
  -e AITRACK_DB_PORT=5432 \
  -e AITRACK_DB_NAME=aitrack \
  -e AITRACK_DB_USER=aitrack \
  -e AITRACK_DB_PASSWORD=$AITRACK_DB_PASSWORD \
  -e AITRACK_ADMIN_KEY=$AITRACK_ADMIN_KEY \
  -e AITRACK_SECRET_KEY=$AITRACK_SECRET_KEY
```

**Go 服务端（轻量备选）：**

```bash
docker compose -f docker/docker-compose.yml --profile go up -d \
  -e DATABASE_URL=postgres://aitrack:${AITRACK_DB_PASSWORD}@db:5432/aitrack \
  -e AITRACK_ADMIN_KEY=$AITRACK_ADMIN_KEY \
  -e AITRACK_SECRET_KEY=$AITRACK_SECRET_KEY
```

### 步骤 6：冒烟验证

```bash
# 1. 服务健康检查（Java）
curl -s http://localhost:8080/actuator/health
# 期望：{"status":"UP"}

# 2. 服务健康检查（Go）
curl -s http://localhost:8081/api/v1/ai-track/stats \
  -H "Authorization: Bearer placeholder"
# 期望：401（服务在运行，token 无效属正常）

# 3. 签发第一个 credential（管理员操作）
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"smoke-test","note":"first-deploy-verify"}'
# 期望：返回 {"credential":"aitrack_...","token_key":"..."}

# 将上一步返回的 credential 存入变量
CREDENTIAL="aitrack_<上一步返回的值>"
TOKEN="${CREDENTIAL%%-*}"

# 4. 通过签发的 token 查询 stats（验证 token 可用）
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"
# 期望：返回空数组 []（尚无数据）
```

### 步骤 7：执行首次 DDL（BM25 + HNSW 索引）

此步骤仅 ParadeDB/PostgreSQL 模式需要，执行一次即可。脚本为幂等操作，重复执行安全。

```bash
# 通过 docker exec 连接 ParadeDB 执行初始化脚本
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack \
  -f /dev/stdin << 'EOF'
-- BM25 全文索引（供 GET /edits/search 使用）
CREATE INDEX IF NOT EXISTS edits_bm25 ON edit_records
  USING bm25 (id, diff_hunk, prompt_summary) WITH (key_field = 'id');

-- HNSW 向量索引（供 POST /edits/similar 使用，仅 embedding 非空行参与）
CREATE INDEX IF NOT EXISTS edits_hnsw ON edit_records
  USING hnsw (embedding vector_cosine_ops) WHERE embedding IS NOT NULL;
EOF
```

完整脚本见 `server-java/src/main/resources/db-postgres-init.sql`。

---

## 4. 数据持久化

### 4.1 H2 模式（开发 / 小团队评估）

H2 数据文件路径（容器内）：

| 服务 | Docker 卷名 | 容器内路径 | 文件 |
|------|------------|-----------|------|
| Java | `aitrack-java-data` | `/app/data` | `aitrack.mv.db` |

**备份命令（停服备份，数据一致性最高）：**

```bash
# Java H2 备份
docker compose -f docker/docker-compose.yml stop aitrack-server-java
docker run --rm \
  -v aitrack-java-data:/data \
  -v $(pwd)/backups:/backup \
  alpine tar czf /backup/h2-$(date +%Y%m%d-%H%M%S).tar.gz /data
docker compose -f docker/docker-compose.yml start aitrack-server-java
```

### 4.2 ParadeDB 模式（生产推荐）

ParadeDB 数据卷：`pgdata`，对应容器内 `/var/lib/postgresql/data`。

**单次备份（手动）：**

```bash
# pg_dump 逻辑备份（不需要停服）
docker compose -f docker/docker-compose.yml exec db \
  pg_dump -U aitrack -d aitrack -Fc \
  > backups/aitrack-$(date +%Y%m%d-%H%M%S).dump
```

**定时备份（cron 示例，每天凌晨 2 点备份，保留 14 天）：**

```cron
0 2 * * * cd /opt/aitrack && docker compose -f docker/docker-compose.yml exec -T db \
  pg_dump -U aitrack -d aitrack -Fc \
  > backups/aitrack-$(date +\%Y\%m\%d).dump \
  && find backups/ -name "aitrack-*.dump" -mtime +14 -delete
```

**恢复备份：**

```bash
# 从 dump 文件恢复（目标库需为空或全新）
docker compose -f docker/docker-compose.yml exec -T db \
  pg_restore -U aitrack -d aitrack -Fc < backups/aitrack-20260101.dump
```

### 4.3 客户端本地数据

| 文件 | 路径 | 说明 |
|------|------|------|
| 配置 | `~/.aitrack/config.toml` | 权限 0600，存储 api_url、credential、device_id |
| 本地记录 | `~/.aitrack/records.db` | 权限 0600，SQLite，已上报记录 synced=1 |

客户端数据在服务端已成功接收（`synced=1`）后无需单独备份。未同步的记录（`synced=0`）由客户端自动重试（最多 5 次），通常无需运维介入。客户端数据丢失不影响服务端已有数据。

---

## 5. 日志与监控

### 5.1 关键日志关键字

| 级别 | 关键字 | 含义 | 处置建议 |
|------|--------|------|----------|
| INFO | `Record accepted` | 正常接受编辑记录 | 无需操作 |
| INFO | `Heartbeat received` | 设备心跳正常 | 无需操作 |
| INFO | `Token issued` | 签发新 credential | 正常流程 |
| WARN | `flagged: diff_inconsistent` | diff 行数与 added/removed_lines 不一致 | 关注频率，偶发可忽略 |
| WARN | `flagged: oversized` | 单条记录超过 added_lines 上限 | 关注是否有工具异常刷行数 |
| WARN | `flagged: repo_unknown` | repo_url 不在白名单 | 白名单模式下需核实是否漏配 |
| WARN | `rate_limited` | 某（token, file_path）触发限流 | 关注是否有异常批量上报 |
| WARN | `rejected: sig_mismatch` | record_sig 验证失败 | 单次偶发可忽略；持续出现说明客户端版本不兼容或数据被篡改 |
| ERROR | `Database connection failed` | 无法连接 ParadeDB | 立即处理，参见第 7 节故障排查 |
| ERROR | `AITRACK_SECRET_KEY not set` | 密钥未配置 | 立即重新注入环境变量并重启 |
| ERROR | `Failed to start` | 服务启动失败 | 查看完整错误，参见第 7 节 |

### 5.2 日志查看命令

```bash
# Java 服务端实时日志
docker logs aitrack-server-java --tail 100 -f

# Go 服务端实时日志
docker logs aitrack-server-go --tail 100 -f

# ParadeDB 日志（数据库层问题排查）
docker logs aitrack-db --tail 100 -f

# 过滤错误日志
docker logs aitrack-server-java 2>&1 | grep -E 'ERROR|WARN|Exception'
```

Spring Boot 日志默认输出到 stdout，不写入宿主机文件。如需持久化日志，在 compose 中添加 logging driver 配置：

```yaml
logging:
  driver: "json-file"
  options:
    max-size: "100m"
    max-file: "10"
```

### 5.3 建议监控指标

| 指标 | 采集方式 | 告警阈值 |
|------|----------|----------|
| 服务端响应时间 P99 | nginx access log / Prometheus | > 2000 ms 告警 |
| POST /edits 成功率 | 服务端日志 accepted / total 比值 | rejected 占比 > 5% 告警 |
| 校验链拒绝率（sig_mismatch） | 日志关键字计数 | 单小时 > 10 次告警 |
| 心跳检测失败设备数 | GET /devices 中 silent=true 数量 | > 0 即告警 |
| ParadeDB 连接池等待时间 | HikariCP JMX / Spring Actuator | > 500 ms 告警 |
| 磁盘使用率（pgdata 卷） | 宿主机 df | > 80% 告警 |
| 客户端 pending_count 积压 | GET /devices 字段 | 单设备 > 100 告警 |

### 5.4 告警阈值建议

| 指标 | 警告阈值 | 严重阈值 | 处置 |
|------|----------|----------|------|
| 服务端无心跳 | 5 分钟 | 15 分钟 | 检查容器状态 |
| 数据库连接失败 | 1 次 | 3 次连续 | 重启 DB 容器 |
| 磁盘使用率 | 80% | 90% | 扩容或清理旧数据 |
| silent 设备数增加 | +1 台 | +5 台 | 联系对应开发者 |

---

## 6. 健康检查

### 6.1 Java 服务端

```bash
# Spring Boot Actuator 健康端点
curl -s http://localhost:8080/actuator/health
# 正常响应: {"status":"UP"}

# 服务端 stats 端点（需有效 token）
TOKEN="aitrack_<你的token>"
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"
# 正常响应: JSON 数组（可以为空 []）
```

### 6.2 Go 服务端

```bash
# Go 服务端 stats 端点（需有效 token）
TOKEN="aitrack_<你的token>"
curl -s "http://localhost:8081/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"
# 正常响应: JSON 数组
```

### 6.3 ParadeDB 健康检查

```bash
# pg_isready 快速检查
docker compose -f docker/docker-compose.yml exec db \
  pg_isready -U aitrack -d aitrack
# 正常输出: localhost:5432 - accepting connections

# 查看数据库连接数和表行数
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "
    SELECT relname, n_live_tup AS rows
    FROM pg_stat_user_tables
    WHERE relname IN ('edit_records','device_heartbeats','tokens')
    ORDER BY relname;
  "
```

### 6.4 客户端状态

```bash
# 在开发者机器上检查钩子安装状态和本地记录
aitrack status
# 正常输出示例：
# hooks: claude=true, codex=false, cursor=false
# pending: 0 records
# last_seen: 2026-05-19T10:00:00Z

# 查看本地最近 10 条记录（确认 capture 正常工作）
aitrack inspect --limit 10
```

---

## 7. 故障处理

### 7.1 服务端无法启动

**症状**：`docker logs` 中出现 `Failed to start` 或容器立即退出（`Exited (1)`）。

**排查命令：**

```bash
# 查看完整启动日志
docker logs aitrack-server-java 2>&1 | tail -50

# 检查端口是否被占用
ss -tlnp | grep -E '8080|8081'
# 或
lsof -i :8080
```

**常见原因与修复：**

| 原因 | 日志特征 | 修复 |
|------|----------|------|
| 端口占用 | `Address already in use: 8080` | `kill $(lsof -t -i:8080)` 后重启；或修改 compose 端口映射 |
| 密钥未配置 | `AITRACK_SECRET_KEY not set` | 确认 `.env` 文件存在且已通过 `export $(cat .env \| xargs)` 注入 |
| 数据库连接失败 | `Connection refused` 或 `FATAL: password authentication failed` | 参见 7.2 |
| 镜像不存在 | `No such image` | 执行 `docker build -f docker/Dockerfile.server-java ...` 重新构建 |

### 7.2 ParadeDB 连接失败

**症状**：服务端日志出现 `Connection refused`、`FATAL: role does not exist` 或 `HikariPool timed out`。

**排查命令：**

```bash
# 1. 检查 ParadeDB 容器状态
docker compose -f docker/docker-compose.yml ps db
# 期望 Status 为 healthy，不是 starting 或 unhealthy

# 2. 手动连接测试
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack -c "SELECT 1;"

# 3. 检查 pgdata 卷挂载权限
docker compose -f docker/docker-compose.yml exec db ls -la /var/lib/postgresql/data

# 4. 查看 DB 容器日志
docker logs aitrack-db --tail 50
```

**常见原因与修复：**

| 原因 | 修复步骤 |
|------|----------|
| DB 容器健康检查未通过（仍在初始化） | 等待 30–60 秒，重新检查 `docker compose ps` |
| pgdata 目录权限异常 | `docker compose exec db chown -R postgres:postgres /var/lib/postgresql/data` |
| 密码不匹配 | 确认 `AITRACK_DB_PASSWORD` 与 DB 容器 `POSTGRES_PASSWORD` 环境变量一致 |
| DB 容器未启动先启动了服务端 | `docker compose up db -d` 等待 healthy 后再启动服务端 |

### 7.3 客户端上报失败（capture 无数据）

**症状**：开发者反馈 `aitrack status` 显示 pending 持续增加，服务端无新记录入库。

**排查命令（开发者机器）：**

```bash
# 1. 检查钩子是否安装
aitrack status

# 2. 手动触发一次心跳并查看输出
aitrack heartbeat --verbose

# 3. 检查网络连通性
curl -v http://aitrack.company.internal/actuator/health

# 4. 查看本地未同步记录
aitrack inspect --limit 5 --synced false
```

**排查命令（服务端）：**

```bash
# 查看是否有来自该设备的 401 或拒绝记录
docker logs aitrack-server-java 2>&1 | grep "sig_mismatch\|401\|rate_limited"
```

**常见原因与修复：**

| 原因 | 修复步骤 |
|------|----------|
| 钩子未安装（`hooks.claude=false`） | `aitrack init --claude --api-url <url> --credential <cred>`；或使用 v1.6.3+ 自动探测模式 `aitrack init --api-url <url> --credential <cred>` |
| credential 过期或被删除 | 管理员重新签发，开发者重新执行 `aitrack init --credential <新cred>` |
| 网络不通 | 确认开发者机器可访问服务端地址；检查防火墙/VPN |
| 客户端版本过旧（协议不兼容） | 升级 aitrack 客户端至 v1.2.0+（credential 合并签发版本） |
| 时钟偏差超过 300 秒 | 同步 NTP：`timedatectl set-ntp true`（Linux）或系统时间设置 |

### 7.4 BM25/ANN 端点返回 501

**症状**：`GET /edits/search` 或 `POST /edits/similar` 返回 `501 Not Implemented`。

**原因**：Java 服务端运行在 H2 模式，未启用 PostgreSQL/ParadeDB。（Go 服务端为 PostgreSQL-only，若出现 501 请检查 DATABASE_URL 是否正确设置。）

**修复步骤（Java）：**

```bash
# 确认当前 profile
docker inspect aitrack-server-java | grep SPRING_PROFILES_ACTIVE
# 如果不含 postgres，需要重启并注入正确环境变量：
docker compose -f docker/docker-compose.yml restart aitrack-server-java \
  -e SPRING_PROFILES_ACTIVE=postgres
```

**修复步骤（Go）：**

```bash
# 确认 DATABASE_URL 是否已设置
docker inspect aitrack-server-go | grep DATABASE_URL
# 如果为空，重启并注入：
docker compose -f docker/docker-compose.yml restart aitrack-server-go \
  -e DATABASE_URL=postgres://aitrack:<password>@db:5432/aitrack
```

验证修复：`GET /edits/search?q=test` 应返回 `{"query":"test","total":0,"hits":[]}`（需有效 Admin Key）。

### 7.5 钩子被静默移除

**症状**：`GET /devices` 返回某设备 `"silent": true`（所有钩子均已移除）。

**排查与处置流程：**

```bash
# 1. 查看问题设备详情
TOKEN="aitrack_<你的token>"
curl -s http://localhost:8080/api/v1/ai-track/devices \
  -H "Authorization: Bearer $TOKEN" \
  | python3 -m json.tool | grep -A 15 '"silent": true'

# 2. 记录 owner、hostname、last_seen 字段

# 3. 联系对应开发者确认情况，要求重新安装钩子：
#    aitrack init --claude --api-url <url> --credential <cred>
#    或使用 v1.6.3+ 自动探测模式（自动检测并安装所有已安装工具的钩子）：
#    aitrack init --api-url <url> --credential <cred>

# 4. 若开发者配合，下次心跳（最长 1 小时）后 silent 恢复 false
# 5. 若持续不恢复，可按安全策略处理（禁用该 token 等）
```

> `silent=true` 本身不会自动触发告警，建议在监控系统中定期轮询 `/devices` 端点并对 `silent=true` 数量设置告警（参见第 5 节）。

### 7.6 H2 数据文件损坏

**症状**：Java 服务端启动时日志出现 `Error opening database`、`Database is corrupted`。

**恢复步骤：**

```bash
# 1. 停止服务端
docker compose -f docker/docker-compose.yml stop aitrack-server-java

# 2. 从最近的备份恢复
docker run --rm \
  -v aitrack-java-data:/data \
  -v $(pwd)/backups:/backup \
  alpine sh -c "rm -rf /data/* && tar xzf /backup/h2-<最近日期>.tar.gz -C /"

# 3. 重启服务端
docker compose -f docker/docker-compose.yml start aitrack-server-java

# 4. 如无可用备份，删除损坏文件（数据丢失）
docker run --rm -v aitrack-java-data:/data alpine rm -rf /data/aitrack.mv.db
docker compose -f docker/docker-compose.yml start aitrack-server-java
```

> H2 损坏通常由未正常停机（`kill -9` 或宿主机断电）引起。生产环境强烈建议切换至 ParadeDB 模式，PostgreSQL 的 WAL 机制对异常断电有更好的恢复能力。

---

## 8. 升级流程

### 8.1 标准升级步骤（零停机建议）

```bash
# 步骤 1：备份数据（PostgreSQL 模式）
docker compose -f docker/docker-compose.yml exec db \
  pg_dump -U aitrack -d aitrack -Fc \
  > backups/pre-upgrade-$(date +%Y%m%d-%H%M%S).dump

# 步骤 2：拉取新镜像（不影响正在运行的容器）
docker compose -f docker/docker-compose.yml pull

# 步骤 3：运行 E2E 验证（使用新镜像，不停止当前服务）
# 在临时端口验证新版本（按团队情况决定是否执行）
# bash e2e/run.sh both

# 步骤 4：滚动切换（停旧容器，启动新容器）
docker compose -f docker/docker-compose.yml --profile java up -d --no-deps aitrack-server-java
# --no-deps 参数确保不重启数据库容器

# 步骤 5：验证升级后状态
curl -s http://localhost:8080/actuator/health
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"
```

### 8.2 Schema 变更说明

| 服务端 | Schema 变更方式 | 注意事项 |
|--------|----------------|----------|
| Java（Spring Boot） | `Hibernate ddl-auto=update` 自动执行 | 新增列、新增表自动处理；禁止删除列，否则需手动迁移 |
| Go | `ALTER TABLE ... IF NOT EXISTS` 幂等脚本 | 服务启动时自动执行初始化 SQL；重复执行安全 |

**v1.3.0 升级特别说明**：新增 `embedding` 和 `prompt_summary` 两列均为可空，无需手动迁移旧数据。BM25 + HNSW 索引需要在升级后手动执行一次（见第 3 节步骤 7）。

### 8.3 版本兼容性矩阵

| 客户端版本 | 服务端最低版本 | 说明 |
|-----------|-------------|------|
| v1.0.x | v1.0.0 | 协议 v1.0 |
| v1.1.x | v1.1.0 | 需服务端支持 hostname 字段 |
| v1.2.x | v1.2.0 | credential 合并签发格式，不兼容旧格式 |
| v1.3.x | v1.2.0+ | 向量/语义检索为可选功能，不影响核心上报 |

### 8.4 禁止操作（历史记录保护）

- 禁止 `git reset --hard`
- 禁止 `git push --force`（无团队书面确认的情况下）
- 禁止 `git commit --amend` 已推送的 commit
- 使用 `git revert` 创建新 commit 撤销变更

---

## 9. 扩缩容

### 9.1 垂直扩容（JVM Heap 调整）

Java 服务端默认 JVM 由容器内存限制决定。如遇 OOM 或 GC 压力：

```yaml
# docker-compose.yml 或 compose override
services:
  aitrack-server-java:
    environment:
      JAVA_OPTS: "-Xmx1g -Xms512m -XX:+UseG1GC"
    mem_limit: 1.5g
```

建议配置：

| 场景 | -Xmx | 容器内存限制 |
|------|------|------------|
| ≤ 10 人 | 256m | 512 MB |
| ≤ 50 人 | 512m | 1 GB |
| ≤ 200 人 | 1g | 2 GB |

### 9.2 水平扩容限制

| 数据库模式 | 水平扩容 | 说明 |
|-----------|----------|------|
| H2（Java） | 不支持 | H2 文件数据库不支持多实例并发写入 |
| ParadeDB/PostgreSQL | 支持 | 服务端无状态，可部署多实例；数据库层共享同一 ParadeDB |

**PostgreSQL 模式多实例启动示例：**

```bash
# 启动两个 Java 服务端实例，连接同一 ParadeDB
docker run -d --name aitrack-java-1 -p 8080:8080 \
  -e SPRING_PROFILES_ACTIVE=postgres \
  -e AITRACK_DB_HOST=db-host \
  aitrack-server-java:latest

docker run -d --name aitrack-java-2 -p 8082:8080 \
  -e SPRING_PROFILES_ACTIVE=postgres \
  -e AITRACK_DB_HOST=db-host \
  aitrack-server-java:latest

# nginx 负载均衡
# upstream aitrack { server 127.0.0.1:8080; server 127.0.0.1:8082; }
```

### 9.3 HikariCP 连接池参数（Java）

在 `application.yml`（或环境变量覆盖）中调整：

```yaml
spring:
  datasource:
    hikari:
      maximum-pool-size: 10       # 最大连接数，建议 = CPU 核数 × 2 + 磁盘数
      minimum-idle: 2              # 最小空闲连接
      connection-timeout: 30000   # 获取连接超时（毫秒）
      idle-timeout: 600000        # 空闲连接超时（毫秒）
      max-lifetime: 1800000       # 连接最大生命周期（毫秒）
```

ParadeDB 默认最大连接数为 100（`max_connections=100`），多实例部署时需确保所有实例的连接池总量不超过该值。如需调整：

```bash
docker compose exec db psql -U aitrack -d aitrack \
  -c "ALTER SYSTEM SET max_connections = 200;"
docker compose restart db
```

---

## 10. 安全加固清单

以下检查项应在首次部署完成后逐一确认，并在每次升级后复核。

- [ ] **AITRACK_ADMIN_KEY 至少 32 字节随机，不使用任何默认值或易猜值**（通过 `openssl rand -hex 32` 生成）

- [ ] **AITRACK_SECRET_KEY 通过环境变量注入，不写入代码或配置文件仓库**（确认 `.env` 已在 `.gitignore` 中）

- [ ] **生产环境 H2 console 已禁用**（Java `application.yml` 中 `spring.h2.console.enabled: false`，非 `dev` profile 下默认即为关闭）

- [ ] **服务端不对公网直接暴露**（通过 nginx/Caddy 反向代理 + TLS 终止；或部署在 VPN 隔离网段内）

- [ ] **反向代理已配置 TLS（HTTPS）**（生产必须；Caddy 可自动申请证书，nginx 需手动配置）

- [ ] **`/admin/**` 接口已通过网络 ACL 或反向代理限制访问**（建议仅允许运维跳板机 IP 访问）

- [ ] **ParadeDB/PostgreSQL 5432 端口不对公网暴露**（通过 Docker 网络隔离，或宿主机防火墙规则）

- [ ] **AITRACK_ADMIN_KEY 定期轮换（建议 90 天）**；轮换步骤：更新 `.env` → 重启服务端 → 通知管理员使用新 Key

- [ ] **旧版 credential 在轮换后及时告知开发者失效**（如有 token 泄露，通过 DELETE /admin/tokens/{id} 撤销）

- [ ] **服务端日志不包含明文 credential 或 hmac_secret**（token 在日志中以 masked 格式 `abcdef…7890` 显示）

- [ ] **客户端 config.toml 和 records.db 权限为 0600**（`ls -la ~/.aitrack/` 确认）

- [ ] **docker compose 文件中不硬编码密码或密钥**（所有敏感值通过 `${ENV_VAR}` 引用）

- [ ] **定期审查 `/devices` 端点中 `silent=true` 的设备并跟进处理**

- [ ] **定期审查 `/edits` 中 `flagged=true` 的记录**，确认是否存在异常刷行数或数据篡改行为

---

## 附录 A：常用运维命令速查

```bash
# 查看所有容器状态
docker compose -f docker/docker-compose.yml ps

# 重启单个服务（不影响其他容器）
docker compose -f docker/docker-compose.yml restart aitrack-server-java

# 完全停止所有容器（保留 volume 数据）
docker compose -f docker/docker-compose.yml down

# 完全清除（含 volume，谨慎：数据不可恢复）
docker compose -f docker/docker-compose.yml down -v

# 查看 ParadeDB 数据库大小
docker compose -f docker/docker-compose.yml exec db \
  psql -U aitrack -d aitrack \
  -c "SELECT pg_size_pretty(pg_database_size('aitrack'));"

# 签发新 token（日常运维）
curl -s -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"<开发者名>","note":"<备注>"}'

# 查询设备心跳状态（管理员日常巡检）
curl -s http://localhost:8080/api/v1/ai-track/devices \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool

# 查询团队月度效能汇总
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN" | python3 -m json.tool
```

---

## 附录 B：环境变量完整参考

| 变量 | 必填 | 默认值 | 说明 |
|------|------|--------|------|
| `AITRACK_ADMIN_KEY` | 是 | 无 | 管理接口鉴权密钥（`openssl rand -hex 32`） |
| `AITRACK_SECRET_KEY` | 是（生产） | 无（开发可省略，明文存储） | AES-256-GCM 密钥（`openssl rand -base64 32`） |
| `SPRING_PROFILES_ACTIVE` | 否 | `default`（H2 模式） | 设为 `postgres` 启用 ParadeDB 模式（Java） |
| `AITRACK_DB_HOST` | 否 | `localhost` | ParadeDB 主机名（Java postgres 模式） |
| `AITRACK_DB_PORT` | 否 | `5432` | ParadeDB 端口（Java postgres 模式） |
| `AITRACK_DB_NAME` | 否 | `aitrack` | 数据库名（Java postgres 模式） |
| `AITRACK_DB_USER` | 否 | `aitrack` | 数据库用户（Java postgres 模式） |
| `AITRACK_DB_PASSWORD` | 是（postgres 模式） | `aitrack_secret`（禁止生产使用默认值） | 数据库密码 |
| `DATABASE_URL` | **是**（Go 服务端） | —（必填，无默认值） | 完整 PostgreSQL/ParadeDB DSN，如 `postgres://aitrack:pass@db:5432/aitrack?sslmode=disable` |
| `AITRACK_TIMESTAMP_WINDOW` | 否 | `300` | 请求时间戳允许偏差（秒） |
| `AITRACK_RATE_LIMIT_PER_HOUR` | 否 | `30` | 每（token, file_path）每小时限流数 |
| `AITRACK_MAX_ADDED_LINES` | 否 | `5000` | 单条记录 added_lines 上限 |
| `AITRACK_REPO_WHITELIST_ENFORCE` | 否 | `false` | 强制拒绝白名单外 repo_url |
| `AITRACK_REPO_WHITELIST_URLS` | 否 | 空 | 允许的 repo URL 前缀，逗号分隔 |
| `JAVA_OPTS` | 否 | 容器默认 | JVM 启动参数，如 `-Xmx512m` |
