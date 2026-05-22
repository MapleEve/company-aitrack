# server-go/internal/ — Go 服务端内部包

六边形架构四层：

| 目录 | 层 | 职责 |
|------|---|------|
| `domain/` | 领域层 | 实体、业务规则（StatsRow 等核心模型） |
| `application/` | 应用层 | Use case、校验链编排 |
| `adapter/` | 适配器层 | HTTP handler（chi）、数据库 repository |
| `infrastructure/` | 基础设施层 | PostgreSQL 连接、pgx/v5 driver |

## 依赖方向

```
adapter → application → domain
infrastructure → adapter（仅 DB 部分）
```

禁止 domain 导入 adapter 或 infrastructure 包。

## 测试策略

- `domain/` + `application/`：纯单元测试，不需要数据库
- `adapter/` + `infrastructure/`：集成测试，使用 `testapp.MemoryConfig` 或真实 PostgreSQL
- E2E：见 `e2e/CLAUDE.md`
