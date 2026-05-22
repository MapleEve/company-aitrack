# server-go/testapp/ — 测试工厂

## 作用域：仅供测试使用

`testapp` 导出两个函数：
- `Build()` — 构建测试用服务器实例
- `MemoryConfig(adminKey string)` — 返回 in-memory SQLite 配置

**`MemoryConfig` 仅用于单元测试和本地链路集成测试（`chain_integration_test.go`）。**

生产环境和 Docker E2E 测试均使用真实 PostgreSQL（`DATABASE_URL`）。

## 禁止事项

- 禁止在非测试代码（`_test.go` 以外）中 import `testapp`
- 禁止将 `MemoryConfig` 用于 E2E 测试（应使用 Docker PostgreSQL）
- 禁止将 SQLite 配置引入 CI 的 coverage job（需 `-coverpkg=./internal/...` + ParadeDB container）
