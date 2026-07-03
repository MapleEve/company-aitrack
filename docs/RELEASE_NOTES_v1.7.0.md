# v1.7.0 发布说明

## 概览

v1.7.0 在保留签名编辑证据模型的基础上，把 aitrack 扩展为「原生编辑证据 + 动态状态心跳 + 本地用量扫描」三层采集模型。

Claude Code、Codex CLI、Cursor 仍是原生编辑钩子的主要证据路径；其他已登记工具可以通过动态状态、心跳、本地会话记录、用量汇总和额度快照进入治理视图。

## 主要变化

- 新增动态工具注册表、状态查询和节流心跳，统一使用规范工具 key 与别名。
- 新增本地用量扫描，覆盖本机会话目录、JSON/JSONL/NDJSON、CSV、SQLite 和本地客户端状态。
- 新增 Java 与 Go 用量接口，支持日汇总、额度/订阅快照和摘要查询。
- 从稳定本地来源中恢复受限的提示词、工具调用、窗口上下文和可还原编辑监控事件。
- 扫描器默认回看 30 天；`--since/--until` 可用于受控回填；每个工具、文件、JSONL/CSV 行数和缓存游标都有上限。
- CI 门禁覆盖架构约束、默认工具矩阵、客户端 E2E 和扫描缓存行为。

## 支持范围

### 原生钩子

| 工具 key | 原生编辑适配器 | 原生提示词钩子 |
|----------|----------------|----------------|
| `claude` | 是 | 是 |
| `codex` | 是 | 是 |
| `cursor` | 是 | 是 |

### 默认本地扫描工具

默认执行 `aitrack usage scan` 或 `aitrack usage sync` 且未指定 `--tool` 时，会扫描以下 30 个规范 key：

`claude`、`codex`、`cursor`、`trae`、`qwen`、`opencode`、`qoder`、`qoder-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`warp`、`zcode`。

显式指定 `--tool` 时，也接受 `roocode`、`kilo-code`、`gajae-code` 作为别名。

## 升级说明

- 本版本没有移除既有服务端写入 API。
- 新增 `/usage/*` 接口沿用 Bearer token 与请求签名模型。
- `EditRecord` 仍是签名监控事件的数据域；用量汇总和额度快照是标量用量数据，不能替代带 diff 的编辑证据。
- 默认本地扫描按工具稳定来源分层处理；没有真实来源证据的登记项不计入默认采集能力。
- 默认本地扫描是增量且有近期窗口限制；历史回填请显式使用 `--since/--until`。

## 验证记录

- Rust 客户端单元测试：301 个通过。
- 本地扫描 E2E 矩阵：v1.7 初版默认工具 key 已覆盖，默认矩阵覆盖率要求 100%。
- 客户端 E2E 缓存断言：未变化数据源立即二次同步时，解析 0 条用量消息和 0 条监控事件。
- PR 门禁覆盖 Rust、Java、Go、架构、覆盖率、Java+Go E2E、Rust 本地扫描 E2E、Codecov、FOSSA 和自动化审查。
