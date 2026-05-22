# docker/ — 容器配置

## 文件结构

| 文件 | 用途 |
|------|------|
| `Dockerfile.client` | Rust 客户端构建（cargo-zigbuild，musl 跨平台） |
| `Dockerfile.server-java` | Java 服务端（maven:3.9-eclipse-temurin-17） |
| `Dockerfile.server-go` | Go 服务端（CGO_ENABLED=0，scratch 镜像） |
| `docker-compose.yml` | 本地开发：ParadeDB + Java + Go |

## docker-compose

```bash
docker compose up -d
```

服务启动顺序：postgres:16-alpine → pg_isready → Java/Go 服务端。

Go 服务端**必须**通过 `DATABASE_URL` 环境变量注入 PostgreSQL 连接串：

```yaml
environment:
  DATABASE_URL: postgres://aitrack:aitrack@db:5432/aitrack?sslmode=disable
```

Java 服务端默认使用 H2，生产环境同样需要 `DATABASE_URL`。

## E2E Docker 流程

E2E 测试中：postgres:16-alpine 先启动 → `pg_isready` 健康检查通过 → Go 服务端启动 → 45 个场景运行。
