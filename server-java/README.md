# aitrack Java 服务端

`server-java` 是 aitrack 服务端的 Java 实现，基于 Spring Boot 3.3.8 和 Java 17。它接收 Rust 客户端上报的签名编辑记录、状态心跳、用量汇总和额度快照，并提供统计、设备状态与查询 API。

## 适用场景

- 希望使用 Spring Boot 运维栈部署 aitrack。
- 需要 H2 快速体验，也可以切换到 PostgreSQL / ParadeDB。
- 需要执行完整的 HMAC 校验链、记录标记和统计查询。

AI 工具支持范围由客户端决定；v1.7.0 的原生钩子、动态心跳和本地用量扫描说明见 [AI 编码工具支持矩阵](../docs/AGENT_SUPPORT.md)。

## 运行

```bash
mvn spring-boot:run
```

默认启动地址：

```text
http://localhost:8080
```

本地开发可使用 H2；生产部署建议切换到 PostgreSQL / ParadeDB，并显式设置管理密钥和加密密钥。

## 切换到 PostgreSQL

在 `application.yml` 中配置数据源：

```yaml
spring:
  datasource:
    url: jdbc:postgresql://localhost:5432/aitrack
    driver-class-name: org.postgresql.Driver
    username: aitrack
    password: secret
  jpa:
    database-platform: org.hibernate.dialect.PostgreSQLDialect
```

如果项目依赖中尚未启用 PostgreSQL driver，请在 `pom.xml` 中加入：

```xml
<dependency>
    <groupId>org.postgresql</groupId>
    <artifactId>postgresql</artifactId>
</dependency>
```

## 配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `aitrack.timestamp-window-seconds` | `300` | 请求 HMAC 防重放窗口，单位秒 |
| `aitrack.rate-limit-per-hour` | `30` | 每个 `(token_key, file_path)` 每小时最大编辑数 |
| `aitrack.max-added-lines` | `5000` | 超大编辑标记阈值 |
| `aitrack.repo-whitelist.enforce` | `false` | 是否硬拒绝未在白名单中的仓库 |
| `aitrack.repo-whitelist.urls` | `[]` | 允许的仓库 URL |
| `aitrack.max-batch-size` | `500` | 单次编辑批量上限 |
| `spring.servlet.multipart.max-request-size` | `8MB` | 请求体上限 |

生产环境应通过部署平台注入 `AITRACK_ADMIN_KEY` 和 `AITRACK_SECRET_KEY`。

## API

| 方法 | 路径 | 鉴权 | 用途 |
|------|------|------|------|
| `POST` | `/admin/tokens` | 管理密钥 | 签发 `credential` 和 `token_key` |
| `POST` | `/api/v1/ai-track/edits` | Bearer + HMAC | 接收编辑记录批量上报 |
| `GET` | `/api/v1/ai-track/edits` | Bearer | 分页查询编辑记录 |
| `POST` | `/api/v1/ai-track/heartbeat` | Bearer + HMAC | 接收设备和工具状态心跳 |
| `GET` | `/api/v1/ai-track/stats` | Bearer | 按 `token`、`repo`、`device`、`hostname`、`tool` 聚合统计 |
| `GET` | `/api/v1/ai-track/devices` | Bearer | 查询设备与工具状态 |
| `POST` | `/api/v1/ai-track/usage/rollup` | Bearer + HMAC | 接收本地用量汇总 |
| `POST` | `/api/v1/ai-track/usage/subscription` | Bearer + HMAC | 接收额度或订阅快照 |

签发示例：

```bash
curl -X POST http://localhost:8080/admin/tokens \
  -H "Content-Type: application/json" \
  -d '{"owner":"alice","note":"laptop"}'
```

返回的 `credential` 只显示一次，客户端后续用它拆分 Bearer token 和 HMAC secret。

## 编辑记录校验链

1. Bearer token 存在且有效。
2. `X-AiTrack-Timestamp` 在允许窗口内。
3. `X-AiTrack-Signature` 与原始请求体匹配。
4. 每条记录的 `record_sig` 与签名字段匹配。
5. `diff_hunk` 行数与 `added_lines` / `removed_lines` 基本一致。
6. 仓库白名单按配置执行标记或拒绝。
7. `file_path` 与仓库信息做合理性检查。
8. 超大编辑被标记。
9. 对同一 `(token_key, file_path)` 执行限流。
10. 已接受和已标记记录入库。

`flagged` 记录会入库，供后续审查；`rejected` 记录不会作为有效编辑计入统计。

## 测试

```bash
mvn test
mvn verify
```

`mvn verify` 会执行 JaCoCo 覆盖率检查。需要容器化验证时可使用 [Docker 说明](../docker/README.md) 和 [E2E 测试说明](../e2e/README.md)。

## 测试结构

```text
src/test/java/com/aitrack/server/
  testkit/                  # 测试工厂
  *ServiceTest.java         # 服务层单元测试
  *ControllerTest.java      # API 层集成测试
  ValidationChainTest.java  # 校验链覆盖
```
