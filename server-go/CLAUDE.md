# server-go/ — Go 服务端

## 概览

chi v5.2.5 / Go 1.25。与 Java 服务端协议完全等价，覆盖低资源部署场景。

## 关键约束

**PostgreSQL-only（v1.6.1 起）**：`modernc.org/sqlite` 依赖已移除。

| 变量 | 说明 |
|------|------|
| `DATABASE_URL` | **必填**，无默认值，例：`postgres://user:pass@host:5432/db?sslmode=disable` |

不设置 `DATABASE_URL` = 服务端拒绝启动。无 SQLite 回退。

## 架构

- `domain/model`：`StatsRow` 及核心实体
- `server-go/testapp/`：导出 `Build()` + `MemoryConfig(adminKey)`，**仅供单元/集成测试使用**
- E2E 测试使用真实 ParadeDB（Docker），不使用 testapp

## 构建与测试

```bash
# 本机构建
CGO_ENABLED=0 go build ./...

# 单元 + 集成测试（需 DATABASE_URL 指向 PostgreSQL 或使用 testapp.MemoryConfig）
go test ./...

# 覆盖率（需 ParadeDB service container）
go test -coverpkg=./internal/... ./...
```

覆盖率目标 ≥ 90%（当前 95.3%，244 tests）。

## 端口

默认 `:8080`。
