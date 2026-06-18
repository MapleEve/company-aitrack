# aitrack Go 服务端

`server-go` 是 aitrack 服务端的 Go 实现，使用 `chi` 和 PostgreSQL / ParadeDB。它与 Rust 客户端和 Java 服务端遵循同一份 [CONTRACT.md](../CONTRACT.md)，可接收签名编辑记录、状态心跳、用量汇总和额度快照。

## 适用场景

- 希望用较小运行时部署 aitrack 服务端。
- 需要 PostgreSQL / ParadeDB 作为唯一数据库。
- 需要与 Java 服务端保持协议兼容，但更偏好 Go 运维栈。

AI 工具支持范围由客户端决定；v1.7.0 的原生钩子、动态心跳和本地用量扫描说明见 [AI 编码工具支持矩阵](../docs/AGENT_SUPPORT.md)。

## 运行

```bash
go build ./...
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .
```

`DATABASE_URL` 必须指向 PostgreSQL 或 ParadeDB。服务启动后默认监听 `8080` 端口。

## 配置

配置通过环境变量传入，不支持 `config.yaml`。

| 环境变量 | 默认值 | 说明 |
|----------|--------|------|
| `DATABASE_URL` | 无 | PostgreSQL / ParadeDB DSN，必填 |
| `AITRACK_PORT` | `8080` | HTTP 监听端口 |
| `AITRACK_SECRET_KEY` | 空 | 用于加密存储 `hmac_secret` 的 AES-256-GCM key，生产环境必填 |
| `AITRACK_ADMIN_KEY` | 空 | `POST /admin/tokens` 的 `X-Admin-Key`，部署前必须设置 |
| `AITRACK_TIMESTAMP_WINDOW` | `300` | 请求 HMAC 防重放窗口，单位秒 |
| `AITRACK_RATE_LIMIT_PER_HOUR` | `30` | 每个 `(token, file_path)` 每小时最大编辑数 |
| `AITRACK_MAX_ADDED_LINES` | `5000` | 超大编辑标记阈值 |
| `AITRACK_REPO_WHITELIST_ENFORCE` | `false` | 是否硬拒绝未在白名单中的仓库 |
| `AITRACK_REPO_WHITELIST_URLS` | 空 | 逗号分隔的允许仓库 URL |
| `AITRACK_MAX_BATCH_SIZE` | `500` | 单次编辑批量上限 |
| `AITRACK_MAX_REQUEST_BODY_BYTES` | `8388608` | 请求体上限，默认 8 MiB |

密钥示例：

```bash
export AITRACK_SECRET_KEY=$(openssl rand -base64 32)
export AITRACK_ADMIN_KEY=$(openssl rand -hex 32)
```

## API

| 方法 | 路径 | 鉴权 | 用途 |
|------|------|------|------|
| `POST` | `/admin/tokens` | `X-Admin-Key` | 签发 `credential` 和 `token_key` |
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
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"owner":"alice","note":"laptop"}'
```

`credential` 只返回一次，请妥善保存到客户端配置。

## 校验链

服务端会对每批编辑记录执行同一套校验：

1. Bearer token 存在且有效。
2. `X-AiTrack-Timestamp` 在允许窗口内。
3. `X-AiTrack-Signature` 与原始请求体匹配。
4. 每条记录的 `record_sig` 与签名字段匹配。
5. `diff_hunk` 行数与 `added_lines` / `removed_lines` 基本一致。
6. 仓库白名单按配置执行标记或拒绝。
7. `file_path` 与仓库信息做合理性检查。
8. 超大编辑被标记。
9. 对同一 `(token, file_path)` 执行限流。
10. 已接受和已标记记录入库。

## 数据库

Go 服务端使用 `pgx/v5` 连接 PostgreSQL / ParadeDB，启动时自动迁移 `tokens`、`edit_records`、`device_heartbeats` 和用量相关表。需要全文检索或向量检索时建议使用 ParadeDB。

## 测试

```bash
go test ./...
go test ./... -coverprofile=cover.out
go tool cover -func=cover.out | tail -1
```

在 macOS 上如果遇到动态链接限制，可使用项目已有的 Docker 构建或 E2E 脚本验证。

## 关键依赖

```text
module github.com/aitrack/server
go 1.25
```

- `github.com/go-chi/chi/v5`
- `github.com/jackc/pgx/v5`
- `gopkg.in/yaml.v3`
