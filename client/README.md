# aitrack 客户端

`aitrack` 是用于本机 AI 编码采集的 Rust 单一二进制客户端。它负责安装原生钩子、捕获编辑证据、生成本地签名记录、扫描本机可见用量来源，并把数据同步到 aitrack 服务端。

## 能力范围

- 原生编辑证据：`claude`、`codex`、`cursor` 适配器可生成带 `diff_hunk`、行数、仓库信息和 `record_sig` 的 `EditRecord`。
- 原生提示词钩子：目前仅 `claude` 支持 `UserPromptSubmit`。
- 动态状态心跳：`aitrack status` 和 `aitrack heartbeat` 会上报本机工具可见性、钩子状态和待同步数量。
- 本地用量扫描：`aitrack usage scan` / `aitrack usage sync` 默认扫描 35 个工具 key；其它工具主要通过本地状态、会话目录、SQLite、CSV、JSON/JSONL/NDJSON 等来源覆盖。

完整工具矩阵、默认 key、别名和扫描边界见 [AI 编码工具支持矩阵](../docs/AGENT_SUPPORT.md)。

## 构建

```bash
cargo build --release
```

构建产物：

```text
target/release/aitrack
```

## 常用命令

```bash
aitrack init --claude --api-url https://aitrack.example.com --credential <credential>
aitrack status
aitrack inspect --limit 20
aitrack heartbeat
aitrack usage scan
aitrack usage sync --api-url https://aitrack.example.com --credential <credential>
aitrack remove --claude
```

`usage scan` 只写入本地用量账本，不上传；`usage sync` 会先扫描，再上传用量汇总、额度快照和本地会话中可还原的监控事件。默认扫描按单轮预算分批推进，返回 `scan_budget_exhausted=true` 时重复运行会继续处理未缓存来源；需要限制范围时可重复指定 `--tool`，也可以用 `--since` / `--until` 做小窗口回填。

## 本地数据

| 路径 | 用途 |
|------|------|
| `~/.aitrack/config.toml` | 保存 `api_url`、`credential`、`device_id` 等配置，权限应为 `0600` |
| `~/.aitrack/records.db` | 保存原生钩子和本地扫描生成的监控记录 |
| `~/.aitrack/usage.sqlite` | 保存本地来源级用量贡献、汇总、额度快照、同步队列和扫描游标；正常扫描不长期保存逐条会话明细 |

客户端只读取本机可见文件和本地状态，不要求用户手动粘贴第三方服务 token。

## 上报协议

编辑记录上报到：

```text
POST {api_url}/api/v1/ai-track/edits
Authorization: Bearer {token}
X-AiTrack-Device: {device_id}
X-AiTrack-Client: aitrack/{version}
X-AiTrack-Timestamp: {unix_seconds}
X-AiTrack-Signature: HMAC_SHA256(hmac_secret, "{ts}\n{sha256(body)}")
```

每条 `EditRecord` 还包含 `record_sig`，覆盖 `token_key`、`device_id`、`hostname`、`timestamp`、`tool`、`file_path`、`repo_url`、`current_sha`、`added_lines`、`removed_lines` 和 `diff_hunk` 摘要。完整协议见 [CONTRACT.md](../CONTRACT.md)。

## 测试

```bash
cargo test
cargo llvm-cov --summary-only
```

如果本机未安装覆盖率工具，可先执行：

```bash
cargo install cargo-llvm-cov
```

真实二进制到服务端的端到端验证见 [E2E 测试说明](../e2e/README.md)。

## 代码结构

```text
src/
  main.rs         # CLI 入口
  lib.rs          # 可复用模块导出
  cli.rs          # 命令行参数
  config.rs       # 本地配置读写
  domain/         # 领域模型、HMAC、diff、关键词分类
  port/           # StoragePort / UploadPort 端口契约
  adapter/        # SQLite、HTTP、原生编辑事件适配
  agent.rs        # 动态工具注册表、默认扫描 key 和别名
  usage/          # 本地用量扫描、汇总、快照和游标缓存
  git.rs          # 仓库信息解析
  heartbeat.rs    # 状态心跳
  init.rs         # 钩子安装和移除
  uploader.rs     # 未同步记录上传
  update.rs       # ed25519 自更新
  testkit/        # 测试工厂
```
