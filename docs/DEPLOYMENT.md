# 部署说明

版本：v1.7.0 · 最后更新：2026-06-18

## 适用对象

这篇面向准备在私有服务器或容器环境中运行 aitrack 服务端的系统管理员。当前重点是 Docker 私有部署和公开版本可直接使用的配置项。

---

## 前提条件

- Docker 20+
- 所有构建命令均从项目根目录 `company-aitrack/` 执行（构建上下文包含三个组件）

---

## 当前版本支持范围

aitrack v1.7.0 支持三层采集模型：

- **原生编辑证据**：`claude`、`codex`、`cursor` 可通过原生编辑适配器生成带签名的编辑记录。
- **提示词与会话来源**：原生提示词钩子仅 `claude` 支持；其他工具主要通过本地可读的会话目录和状态文件提取提示词、工具调用、窗口上下文、可还原编辑事件和用量标量。
- **动态心跳与本地用量扫描**：心跳可上报已登记工具状态；默认本地扫描覆盖 35 个规范工具 key。

纯 token、额度或订阅快照只进入用量数据域，不会被当作编辑监控事件上传。

---

## 服务端镜像构建（两个实现）

每个 Dockerfile 均为多阶段构建：构建阶段运行测试并强制 90% 覆盖率门槛，通过后产出最小运行时镜像。

### Java 服务端镜像

```bash
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .
```

- 构建阶段：`maven:3.9-eclipse-temurin-17`，执行 `mvn verify`（含 JaCoCo LINE ≥ 90% 门槛）
- 运行时镜像：`eclipse-temurin:17-jre`，监听端口 8080，H2 数据库持久化到 `/app/data`

### Go 服务端镜像

```bash
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

- 构建阶段：`golang:1.25`，执行 `go test ./...` + 覆盖率门槛 + `CGO_ENABLED=0 go build`
- 运行时镜像：`gcr.io/distroless/base-debian12`（无 shell），监听端口 8080

---

## 启动服务端

使用 docker compose 按需选择实现。同一主机上建议只运行一个实现。

### 启动 Java 服务端（端口 8080）

```bash
docker compose -f docker/docker-compose.yml --profile java up -d
```

### 启动 Go 服务端（端口 8081）

```bash
docker compose -f docker/docker-compose.yml --profile go up -d
```

> Go 服务端依赖 `db`（ParadeDB）服务，`--profile go` 会自动启动 ParadeDB 容器（健康检查通过后再启动 Go 服务）。

---

## 配置项

### 必须配置（生产环境）

| 环境变量 | 说明 |
|----------|------|
| `AITRACK_ADMIN_KEY` | 管理接口鉴权密钥，用于调用 `POST /admin/tokens`。**生产环境必须修改默认值。** 生成：`openssl rand -hex 32` |
| `AITRACK_SECRET_KEY` | AES-256-GCM 密钥，用于加密存储 `hmac_secret`。Base64 编码的 32 字节。**生产环境必须设置。** 生成：`openssl rand -base64 32` |

开发环境下 `AITRACK_SECRET_KEY` 可不设置，`hmac_secret` 将以 `plain:` 前缀明文存储。

### 可调整的业务参数

以下参数两套服务端实现均支持，Go 实现通过环境变量或 `config.yaml` 配置，Java 实现通过 `application.yml` 配置。

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `AITRACK_TIMESTAMP_WINDOW` / `aitrack.timestamp-window-seconds` | `300` | 请求时间戳允许偏差（秒），超出则 401 |
| `AITRACK_RATE_LIMIT_PER_HOUR` / `aitrack.rate-limit-per-hour` | `30` | 每 (token, file_path) 每小时最多接受的编辑数 |
| `AITRACK_MAX_ADDED_LINES` / `aitrack.max-added-lines` | `5000` | 单条记录 added_lines 上限，超出则 flagged: oversized |
| `AITRACK_REPO_WHITELIST_ENFORCE` / `aitrack.repo-whitelist.enforce` | `false` | 是否强制拒绝白名单外的 repo_url |
| `AITRACK_REPO_WHITELIST_URLS` / `aitrack.repo-whitelist.urls` | 空 | 允许的 repo URL 列表（逗号分隔 / YAML 列表） |

### 通过 .env 文件配置

```bash
# .env
AITRACK_ADMIN_KEY=your-secure-admin-key
AITRACK_SECRET_KEY=your-base64-32-byte-key
AITRACK_RATE_LIMIT_PER_HOUR=30
AITRACK_MAX_ADDED_LINES=5000
```

启动前导入：

```bash
export $(cat .env | xargs)
docker compose -f docker/docker-compose.yml --profile java up -d
```

---

## 数据卷

| 服务 | 卷名 | 容器内路径 | 说明 |
|------|------|-----------|------|
| server-java | `aitrack-java-data` | `/app/data` | H2 数据库文件 |
| db (ParadeDB) | `pgdata` | `/var/lib/postgresql/data` | ParadeDB / PostgreSQL 数据 |

删除卷（谨慎，将丢失所有数据）：

```bash
docker compose -f docker/docker-compose.yml down -v
```

---

## Java 服务端切换 ParadeDB/PostgreSQL

Java 服务端默认使用 H2 嵌入式数据库（适合评估和小团队）。切换为 ParadeDB/PostgreSQL：

```bash
# 确保 ParadeDB 已启动
docker compose -f docker/docker-compose.yml up db -d

# 以 postgres profile 启动 Java 服务端
SPRING_PROFILES_ACTIVE=postgres \
AITRACK_DB_HOST=localhost \
AITRACK_DB_PORT=5432 \
AITRACK_DB_NAME=aitrack \
AITRACK_DB_USER=aitrack \
AITRACK_DB_PASSWORD=aitrack_secret \
docker compose -f docker/docker-compose.yml --profile java up -d
```

或在 `.env` 中配置后启动：

```bash
# .env
AITRACK_ADMIN_KEY=your-secure-admin-key
AITRACK_SECRET_KEY=your-base64-32-byte-key
SPRING_PROFILES_ACTIVE=postgres
AITRACK_DB_HOST=localhost
AITRACK_DB_PORT=5432
AITRACK_DB_NAME=aitrack
AITRACK_DB_USER=aitrack
AITRACK_DB_PASSWORD=aitrack_secret

export $(cat .env | xargs)
docker compose -f docker/docker-compose.yml --profile java up -d
```

新增环境变量（Java postgres profile）：

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `SPRING_PROFILES_ACTIVE` | *(空 = H2)* | 设为 `postgres` 启用 PostgreSQL 模式 |
| `AITRACK_DB_HOST` | `localhost` | 数据库主机 |
| `AITRACK_DB_PORT` | `5432` | 端口 |
| `AITRACK_DB_NAME` | `aitrack` | 数据库名 |
| `AITRACK_DB_USER` | `aitrack` | 用户名 |
| `AITRACK_DB_PASSWORD` | `aitrack_secret` | 密码 |

**新增环境变量（Go 服务端）：**

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `DATABASE_URL` | *(必填)* | PostgreSQL / ParadeDB DSN，如 `postgres://user:pass@host:5432/db?sslmode=disable`。**Go 服务端必须提供 PostgreSQL/ParadeDB DSN。** |

---

## 快速启动（H2，开发/评估环境）

```bash
# 1. 克隆仓库
git clone <repo-url>
cd company-aitrack

# 2. 生成密钥
export AITRACK_SECRET_KEY=$(openssl rand -base64 32)
export AITRACK_ADMIN_KEY=$(openssl rand -hex 32)

# 3. 构建并启动
docker compose -f docker/docker-compose.yml --profile java up -d --build

# 4. 验证服务
curl http://localhost:8080/actuator/health

# 5. 签发第一个 token
curl -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"macbook-dev"}'
# → {"credential":"aitrack_...-<hmac_secret>","token_key":"..."}

# 6. 开发者初始化客户端
aitrack init --claude \
  --api-url http://localhost:8080 \
  --credential <credential>
```

`claude`、`codex`、`cursor` 支持原生编辑适配器，可按实际使用工具选择对应参数，例如 `--claude`、`--codex` 或 `--cursor`。其他已登记工具可通过本地用量扫描和心跳状态纳入可见范围。

---

## 客户端构建与分发

### 本地构建

```bash
cd client
cargo build --release
# 产物：client/target/release/aitrack
```

### 跨平台构建

```bash
# 推荐：直接从 GitHub Releases 下载预构建二进制
# https://github.com/MapleEve/company-aitrack/releases/latest
#
# 已安装后，使用 aitrack update 自动更新到最新版本

# --- 本地手动构建（贡献者 / 自定义场景）---

# macOS Apple Silicon (aarch64)
cargo build --release --target aarch64-apple-darwin

# macOS Intel (x86_64)
cargo build --release --target x86_64-apple-darwin

# Linux x86_64 musl（静态链接，无运行时依赖）
# 需要 cargo-zigbuild + ziglang：pip install ziglang && cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-musl

# Linux aarch64 musl（树莓派 4 / ARM 服务器）
cargo zigbuild --release --target aarch64-unknown-linux-musl

# Windows x86_64
cargo build --release --target x86_64-pc-windows-msvc

# Windows ARM64
cargo build --release --target aarch64-pc-windows-msvc
```

### 安装位置

将 `aitrack` 二进制分发给开发者，建议放在：
- macOS / Linux：`/usr/local/bin/aitrack`
- Windows：`%LOCALAPPDATA%\Programs\aitrack\aitrack.exe`（或任意 PATH 目录）

开发者安装完成后执行：
```bash
aitrack init --claude --api-url https://aitrack.example.com \
             --credential <credential>
```

如果团队使用 Codex CLI 或 Cursor，可改用对应参数安装原生编辑钩子：

```bash
aitrack init --codex --api-url https://aitrack.example.com \
             --credential <credential>
aitrack init --cursor --api-url https://aitrack.example.com \
             --credential <credential>
```

---

## 运维要点

### 日志查看

Spring Boot 默认日志到 stdout，通过 docker logs 查看：

```bash
docker logs aitrack-server --tail 100 -f
```

关键日志关键字：
- `校验链失败` / `sig_mismatch` — 签名验证问题
- `rate_limited` — 限流触发
- `flagged` — 可疑记录被标记
- `ERROR` / `WARN` — 需要关注的异常

### Admin 接口安全

生产环境中 `/admin/**` 应通过网络 ACL 或反向代理限制访问，不对公网暴露。

### Token 签发流程

```bash
curl -X POST http://localhost:8080/admin/tokens \
  -H 'X-Admin-Key: YOUR_ADMIN_KEY' \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"dev machine"}'
# 响应中的 credential 仅出现一次，立即将其值交给对应开发者
```

### 检查设备心跳状态

```bash
curl http://localhost:8080/api/v1/ai-track/devices \
  -H 'Authorization: Bearer aitrack_...'
```

`hooks` 是动态工具状态映射；原生钩子或已登记工具状态异常的设备需人工核查是否绕过了监控。

### 数据备份（H2）

```bash
# 停服备份
docker compose -f docker/docker-compose.yml stop aitrack-server
docker run --rm -v aitrack-java-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/aitrack-data-$(date +%Y%m%d).tar.gz /data
docker compose -f docker/docker-compose.yml start aitrack-server
```

### 数据备份（ParadeDB / PostgreSQL）

```bash
# pg_dump 方式备份（Go 服务端或 Java postgres 模式）
docker exec <paradedb-container-name> pg_dump -U aitrack aitrack > aitrack-$(date +%Y%m%d).sql

# 恢复
docker exec -i <paradedb-container-name> psql -U aitrack aitrack < aitrack-20260101.sql
```

`pgdata` 卷备份（整个数据目录）：

```bash
docker compose -f docker/docker-compose.yml stop db
docker run --rm -v pgdata:/data -v $(pwd):/backup \
  alpine tar czf /backup/pgdata-$(date +%Y%m%d).tar.gz /data
docker compose -f docker/docker-compose.yml start db
```

### 升级流程

```bash
# 1. 拉取新镜像
docker compose -f docker/docker-compose.yml pull

# 2. 重启（H2 数据持久化在 volume，安全）
docker compose -f docker/docker-compose.yml --profile java up -d

# 3. 验证
curl http://localhost:8080/actuator/health
```

### HTTPS 配置（生产必须）

推荐在服务器前置 nginx/Caddy 做 TLS 终止：

```nginx
# /etc/nginx/conf.d/aitrack.conf
server {
    listen 443 ssl;
    server_name aitrack.example.com;

    ssl_certificate     /etc/letsencrypt/live/aitrack.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/aitrack.example.com/privkey.pem;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

使用 Caddy（自动 HTTPS）：
```
aitrack.example.com {
    reverse_proxy localhost:8080
}
```

### H2 控制台（仅开发环境）

H2 控制台在非 `dev` profile 下默认关闭，生产部署无需额外配置。本地开发时访问：

```
http://localhost:8080/h2-console
JDBC URL: jdbc:h2:file:./data/aitrack
User: sa
Password:（空）
```

**生产环境务必禁用**，在 `application.yml` 中设置：
```yaml
spring.h2.console.enabled: false
```

---

## 资源规格建议

| 规模 | 建议配置 | 存储（1 年） |
|------|----------|-------------|
| 小团队（1–10 人） | 1 vCPU / 512MB RAM | < 1GB（H2 足够） |
| 中等团队（10–50 人） | 2 vCPU / 1GB RAM | 1–10GB（建议 PostgreSQL） |
| 大型团队（50+ 人） | 4 vCPU / 2GB RAM + 独立 PG | 10–100GB |

估算基准：每位开发者每天约 50–200 次编辑事件，每条记录约 2–5KB（含 diff_hunk）。

---

## E2E 验证

部署完成后可运行 e2e 测试套件验证整体链路：

```bash
# 对 Java 和 Go 实现各跑一轮
bash e2e/run.sh both

# 使用真实 Rust 二进制做端到端验证
bash e2e/run-client-e2e.sh both
```

详见 `e2e/README.md`。
