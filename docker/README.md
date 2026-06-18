# aitrack Docker

`docker/` 目录提供客户端、Java 服务端和 Go 服务端的多阶段镜像构建文件，以及本地体验和 E2E 验证用的 Compose 配置。所有构建命令都应在仓库根目录执行。

## 镜像构建

### Rust 客户端

```bash
docker build -f docker/Dockerfile.client -t aitrack-client:latest .
```

构建过程会执行 `cargo build --release` 和客户端测试，运行时镜像包含 `/usr/local/bin/aitrack`。

### Java 服务端

```bash
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .
```

构建过程会执行 `mvn verify`，运行时使用 Java 17，默认监听 `8080`。

### Go 服务端

```bash
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

构建过程会执行 Go 测试和覆盖率检查，运行时默认监听 `8080`。

## 本地启动

### Java 服务端

```bash
docker compose -f docker/docker-compose.yml --profile java up -d
```

服务地址：

```text
http://localhost:8080
```

### Go 服务端

```bash
docker compose -f docker/docker-compose.yml --profile go up -d
```

服务地址：

```text
http://localhost:8081
```

## 环境变量

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `AITRACK_ADMIN_KEY` | `dev-admin-key-change-in-prod` | 调用 `POST /admin/tokens` 时使用 |
| `AITRACK_SECRET_KEY` | 空 | 用于加密存储 `hmac_secret`，生产环境必须设置 |
| `DATABASE_URL` | Compose 默认值 | Go 服务端连接 PostgreSQL / ParadeDB 时使用 |

生产环境请覆盖默认管理密钥，并使用：

```bash
openssl rand -hex 32
openssl rand -base64 32
```

分别生成管理密钥和 AES-256-GCM key。

## E2E 验证

```bash
bash e2e/run.sh both
bash e2e/run.sh java
bash e2e/run.sh go
```

真实客户端二进制验证：

```bash
bash e2e/run-client-e2e.sh both
```

E2E 会覆盖签发 `credential`、编辑记录上报、状态心跳、统计查询和设备状态查询。v1.7.0 的工具支持范围以 [AI 编码工具支持矩阵](../docs/AGENT_SUPPORT.md) 为准。

## 数据卷

| 服务 | 数据卷 | 容器路径 |
|------|--------|----------|
| `server-java` | `aitrack-java-data` | `/app/data` |
| `db` | `pgdata` | `/var/lib/postgresql/data` |

清理本地容器和数据卷：

```bash
docker compose -f docker/docker-compose.yml down -v
```
