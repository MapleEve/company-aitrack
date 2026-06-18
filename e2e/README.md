# aitrack E2E 测试

`e2e/` 覆盖从管理端签发 `credential` 到客户端上报、服务端校验、状态心跳、统计查询和设备查询的完整链路。测试同时跑 Java 服务端和 Go 服务端，用于确认两端协议兼容。

## 目录结构

```text
e2e/
  fixtures/prompts/       # 测试载荷使用的代码片段
  factory/factory.go      # 可复现的请求构造器
  scenarios/runner.go     # Go 场景运行器
  Dockerfile.e2e          # 运行器镜像
  run.sh                  # 容器化场景测试入口
  run-client-e2e.sh       # 真实 Rust 客户端链路测试入口
  go.mod
```

## 容器化场景测试

在仓库根目录执行：

```bash
bash e2e/run.sh both
bash e2e/run.sh java
bash e2e/run.sh go
```

脚本会依次构建镜像、启动目标服务端、运行场景、清理容器，并输出每个实现的 `PASS` / `FAIL`。

覆盖场景：

| 场景 | 覆盖内容 |
|------|----------|
| 管理端签发 | 错误管理密钥、缺少 owner、正常签发 |
| 请求鉴权 | 缺少 Bearer、错误 token、过期时间戳、错误 HMAC |
| 编辑上报 | `POST /edits`、`GET /edits`、`/stats`、`/devices` |
| 防篡改 | 错误 `record_sig`、超大编辑、缺少字段 |
| 状态心跳 | `POST /heartbeat` 后设备状态可查询 |
| 仓库白名单 | 未开启强制模式时未知仓库被接受或标记 |

## 真实客户端 E2E

`run.sh` 使用 Go 运行器直接构造签名请求；`run-client-e2e.sh` 会编译并执行真实 `aitrack` 二进制，覆盖本地捕获到服务端接收的完整路径。

```bash
bash e2e/run-client-e2e.sh both
bash e2e/run-client-e2e.sh java
bash e2e/run-client-e2e.sh go
```

真实客户端 E2E 会检查：

1. `cargo build --release` 成功。
2. 服务端可签发新的 `credential`。
3. 脚本使用独立 `AITRACK_HOME`，不读写真实 `~/.aitrack/`。
4. 临时 git 仓库可提供 `repo_url`、`branch` 和 `current_sha`。
5. `claude`、`codex`、`cursor` 三个原生编辑适配器都能通过 `aitrack capture --tool <tool>` 写入本地记录。
6. 本地 `records.db` 中的 `record_sig` 为有效 HMAC，且记录已同步。
7. 服务端 `GET /api/v1/ai-track/edits` 能查询到对应记录。
8. `GET /api/v1/ai-track/stats` 和 `GET /api/v1/ai-track/devices` 反映上报结果。

v1.7.0 的默认本地用量扫描范围、原生提示词钩子范围和动态心跳语义见 [AI 编码工具支持矩阵](../docs/AGENT_SUPPORT.md)。E2E 重点验证协议链路，不重复完整工具矩阵。

## 本机依赖

- `docker`
- `cargo`
- `sqlite3`
- `curl`
- `git`
- `python3`
- `uuidgen`

## Compose 入口

```bash
docker compose -f docker/docker-compose.e2e.yml --profile java up --abort-on-container-exit
docker compose -f docker/docker-compose.e2e.yml --profile go up --abort-on-container-exit
```

## 测试载荷

`fixtures/prompts/` 中的片段用于生成可复现的 diff 和请求内容：

- `claude_edit_snippet.txt`
- `codex_edit_snippet.txt`
- `cursor_edit_snippet.txt`

`factory/factory.go` 提供默认编辑、心跳、签名、篡改记录和超大编辑等构造器，保证场景输入稳定可重复。
