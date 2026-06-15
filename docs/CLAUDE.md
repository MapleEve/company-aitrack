# docs/

## 变更触发矩阵

修改代码时，必须同步更新以下文档：

| 变更类型 | 必须更新 |
|---------|---------|
| 新增 API 端点 | `API.md` |
| 修改部署配置 / 环境变量 | `DEPLOYMENT.md` |
| 修改协议字段（CONTRACT.md） | `API.md` + `ARCHITECTURE.md` |
| 修改数据库 schema | `ARCHITECTURE.md` |
| 修改认证 / 安全机制 | `SECURITY_MODEL.md` |
| 修改测试策略 / 框架 | `TESTING.md` |
| 新版本发布 | `ROADMAP.md` |
| 本地开发流程变更 | `DEVELOPMENT.md` |

## 文件职责

| 文件 | 受众 | 内容 |
|------|------|------|
| `API.md` | 集成方 | 所有 HTTP 端点、请求/响应格式、错误码 |
| `ARCHITECTURE.md` | 贡献者 | 系统设计、数据流、技术选型 |
| `DEPLOYMENT.md` | 运维 | Docker、环境变量、生产配置 |
| `DEVELOPMENT.md` | 贡献者 | 本地开发、构建、调试 |
| `SECURITY_MODEL.md` | 安全审查 | 威胁模型、HMAC 机制、隐私边界 |
| `TESTING.md` | 贡献者 | 测试分层策略、覆盖率要求 |
| `ROADMAP.md` | 贡献者 | 版本计划（v1.x–v2.0） |
| `PRIVACY.md` | 合规 | 数据采集范围、本地存储、隐私保障 |
