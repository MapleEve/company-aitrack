# e2e/ — E2E 集成测试

## 运行

```bash
bash e2e/run.sh
```

Docker 必须在运行。Java 和 Go 服务端各跑一轮。

## 关键约束

- **E2E 不修改真实编辑器配置**：所有操作在容器隔离环境，不触碰 `~/.aitrack/`、`~/.claude/` 等目录
- E2E 使用真实 ParadeDB（postgres:16-alpine），不使用 SQLite 或 H2
- Go 服务端 E2E 需 `DATABASE_URL` 环境变量指向 ParadeDB 容器

## chain_integration_test.go

`chain_integration_test.go` 是 Go router + in-memory SQLite 的**本地链路集成测试**，不需要 Docker，不是 Docker E2E。

## 覆盖场景

45 个 E2E 场景，覆盖从签发 Token 到统计查询的完整链路。
