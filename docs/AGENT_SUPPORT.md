# AI 编码工具支持矩阵

## 适用对象

这篇面向需要评估 aitrack 当前采集范围的管理员、开发者和安全审查者。

---

## 一句话结论

aitrack v1.8.0 已从三条固定编辑钩子扩展为「原生编辑证据 + 动态状态心跳 + 本地用量扫描」三层采集模型：

- **原生编辑证据**：Claude Code、Codex CLI、Cursor 可以通过原生编辑钩子生成带签名的 `EditRecord`。
- **动态状态心跳**：所有已登记工具都可以出现在 `hooks` 动态状态图里，用于排查工具是否存在、钩子是否异常。
- **本地用量扫描**：默认会扫描 35 个规范工具 key。扫描器按每个工具的本地记录结构读取提示词、助手输出、工具调用、工具结果、用量、会话时间、模型信息和编辑线索，并用多个本地来源合并成 agent 级完整数据面。

---

## 数据域边界

| 数据域 | 用途 | 可以进入的来源 | 不能混用的内容 |
|--------|------|----------------|----------------|
| `EditRecord` 监控事件 | 可信编辑证据、提示词/工具/窗口监控事件 | 原生编辑钩子；本地会话记录中可还原的提示词、工具调用、窗口和编辑事件 | 只有 token 数、请求数、成本估算的纯用量数据 |
| 状态心跳 | 设备状态、工具注册状态、钩子异常排查 | `aitrack status`、`aitrack heartbeat`、每次采集后的节流心跳 | 不能等同于某个工具已经有原生编辑钩子 |
| 用量汇总 | 按天、工具、模型、账号汇总 token、消息数和成本估算 | 来源提供的用量字段；本地 transcript / hook 文本派生的用量 | 不能伪装成签名编辑记录；上报会保留用量依据，方便区分来源计量和本地派生计量 |
| 额度/订阅快照 | 保存本地可读取的剩余额度、套餐和重置时间 | Claude Code、Codex CLI 可读取的本地状态 | 不能要求用户手动粘贴第三方云端 token |

---

## 原生钩子支持

| 工具 key | 原生编辑钩子 | 原生提示词钩子 | 说明 |
|----------|--------------|----------------|------|
| `claude` | 是 | 是 | 支持文件编辑证据、提示词上下文、本地会话记录扫描、用量汇总和本地额度快照 |
| `codex` | 是 | 是 | 支持文件编辑证据、提示词上下文、本地会话记录扫描、用量汇总和本地会话额度快照 |
| `cursor` | 是 | 是 | 支持文件编辑证据、提示词上下文、Cursor 本地状态扫描和用量汇总 |

原生编辑钩子是最强证据路径，因为它可以直接生成带 diff、行数、仓库信息和 `record_sig` 的文件编辑类 `EditRecord`。

---

## 默认本地扫描范围

默认执行 `aitrack usage scan` 或 `aitrack usage sync` 时，未指定 `--tool` 会扫描以下 35 个规范 key：

`claude`、`codex`、`cursor`、`trae`、`qwen`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode`。

显式指定 `--tool` 时，还接受以下别名：

| 别名 | 归并到 |
|------|--------|
| `roocode` | `roo-code` |
| `kilo-code` | `kilocode` |
| `gajae-code` | `gjc` |

默认扫描和 registered/default coverage 只使用规范 key，避免同一个本地目录被重复读取；别名只用于 CLI、扫描和探测入口的输入归并。

---

## 可以读取的本地来源

aitrack 只从本机可见的文件和本地状态读取，不要求用户另行登录第三方账号，也不要求手动粘贴第三方服务 token。通用入口包括：

- 工具默认配置目录和会话目录。
- `~/.aitrack/local-sources/<tool>`。
- `~/.aitrack/sources/<tool>`。
- JSON、JSONL、NDJSON、CSV、SQLite 数据库。
- 工具本地状态、本地导出的会话文件。

当来源中存在对应字段时，扫描器会尽量提取：

| 字段类型 | 进入的数据域 |
|----------|--------------|
| 用户提示词、助手输出、工具调用、工具结果、窗口上下文 | 监控事件域 |
| token 输入、输出、缓存读写、推理 token、消息数、成本估算 | 用量汇总域 |
| 会话剩余额度、周额度、重置时间、套餐信息 | 额度/订阅快照域 |
| 可还原文件编辑事件、文件路径、diff 或编辑摘要 | 监控事件域 |

### 默认来源能力分层

不同工具的本地入口不一样，默认扫描按每个来源真实存在的字段处理，再按 agent 合并为完整上报数据。source 行只说明字段来自哪个本地结构，不把一个来源没有的字段写成该来源自己的能力。

| 来源级别 | 来源 id | 当前处理方式 |
|----------|---------|--------------|
| 原生钩子 | `claude`、`codex`、`cursor` | 通过本地 hook 捕获编辑证据和提示词上下文，是文件编辑证据的最强路径 |
| Agent 合并完整覆盖 | `claude`、`codex`、`cursor`、`trae`、`qwen`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode` | 每个默认 key 都有本地 source 组合覆盖提示词、助手输出、工具调用、工具结果、用量、会话和时间字段；模型、成本、推理 token、缓存和编辑字段按来源真实字段进入上报 |
| 字段级原生读取来源 | `claude/projects-jsonl`、`codex/rollout-jsonl`、`trae/trajectory-json`、`qwen/project-chats-jsonl`、`opencode/sqlite`、`kiro/data-sqlite`、`cline/vscode-ui-messages`、`roo-code/vscode-ui-messages`、`hermes/sqlite`、`openclaw/session-jsonl`、`gjc/session-jsonl`、`zed/threads-db`、`goose/sessions-db`、`pi/session-jsonl`、`mux/chat-jsonl`、`mux/session-usage-json`、`crush/sqlite`、`kilo/sqlite`、`kilo/storage-json`、`kilocode/sqlite`、`kilocode/storage-json`、`droid/session-jsonl`、`kimi/wire-jsonl`、`gemini/tmp-chats-jsonl`、`copilot/official-copilot-runtime-jsonl`、`codebuff/project-jsonl`、`synthetic/sqlite`、`warp/warp-sqlite`、`antigravity/conversation-sqlite`、`wukong/sqlite` | 扫描器读取本地记录里明确存在的原生字段，例如 credits、cost、总 token、分类 token、会话、工具调用、工具结果或编辑线索；不把总量冒充输入/输出/cache/reasoning 分桶，也不把工具参数派生的编辑写成来源自带编辑快照 |
| 本地派生读取来源 | `cursor/hook-jsonl`、`cursor/agent-transcripts-jsonl`、`kiro/hook-jsonl`、`amp/threads-jsonl`、`qoder/transcript-jsonl`、`qoder-cn/transcript-jsonl`、`qoder-work/trace-jsonl`、`qoder-work-cn/trace-jsonl`、`grok/sessions-jsonl`、`zcode/projects-jsonl` | 来源具备本地 transcript、hook、trace 或会话事件字段，可读取提示词、输出、工具调用、工具结果、编辑和会话上下文中的已存在部分；用量以 `usage_basis=local_derived` 标记，按本地文本或事件内容估算 |
| 辅助状态/用量来源 | `cursor/state-vscdb`、`opencode/export-json`、`qwen/telemetry-log`、`qwen/usage-record-jsonl`、`qwen/token-usage-jsonl`、`gemini/telemetry-log`、`copilot/otel-jsonl`、`copilot/session-state-jsonl`、`copilot/session-store-db`、`copilot/vscode-chat-state`、`qoder/hook-jsonl`、`qoder/local-db`、`qoder-cn/hook-jsonl`、`qoder-cn/local-db`、`qoder-work/hook-jsonl`、`qoder-work/local-db`、`qoder-work-cn/hook-jsonl`、`qoder-work-cn/local-db`、`cline/vscode-tasks`、`cline/sessions-db`、`roo-code/vscode-tasks`、`kiro/cli-session-json`、`kilocode/vscode-tasks` | 用于补齐本地用量、会话、状态或遥测线索；只按来源明确提供的字段进入用量汇总或监控事件 |

`usage_basis=native` 表示用量来自来源提供的 token、成本或等价用量字段；`usage_basis=local_derived` 表示用量由本地 transcript / hook 文本按稳定规则估算。二者都可用于员工监控统计；上报会保留来源类型，方便管理员区分来源计量和本地派生计量。

同一工具可能同时有字段级原生读取来源、本地派生读取来源和辅助状态/用量来源，例如 `opencode/sqlite` 与 `opencode/export-json`、`qwen/project-chats-jsonl` 与 `qwen/telemetry-log` / `qwen/token-usage-jsonl`、`gemini/tmp-chats-jsonl` 与 `gemini/telemetry-log`、`copilot/official-copilot-runtime-jsonl` 与 `copilot/otel-jsonl` / `copilot/session-store-db` 分别按不同级别处理。Copilot runtime JSONL 从 `.copilot/session-state/<session>/events.jsonl` 按事件名读取用户消息、助手消息或增量、工具执行开始和完成、用量、会话上下文与关闭时的代码变更；其它 Copilot 本地缓存和状态来源用于补齐会话与状态字段。Gemini ChatRecording JSONL 支持提示词、助手输出、工具调用、工具结果、token 和会话上下文字段级读取；Gemini telemetry log 提供用量和工具遥测。Trae trajectory JSON 按文件内存在的任务、交互、工具和 usage 字段读取。Cline/Roo Code 的任务记录按 transcript 覆盖提示词、助手输出、工具调用、工具结果和会话上下文；发现明确的本地 metrics 时会读取 token/cost。Kilo/KiloCode 同时支持 `kilo.db` 和 `storage/session + storage/message + storage/part + storage/session_diff` 分片结构；storage JSON 只以 session 文件为入口聚合对应 message、part 和 diff，避免重复扫描。Kiro hook JSONL 可按事件字段读取提示词、助手输出、工具调用、工具结果和文件编辑事件；该来源的上报覆盖来自本地派生用量。Warp 本地 SQLite 的 token 元数据可能提供总量或分类总量，而不是输入/输出分桶；扫描器只保留来源明确提供的 token 总量/分类总量和 credits/cost。Codebuff usage 以本地 run-state、chat message credits 和 source_cost 字段为准，不要求来源提供稳定 token bucket。Synthetic/Octofriend 与 Wukong 只按本地结构中明确存在的字段读取。字段缺失不补假数据，也不会用 token、成本或请求数反推出提示词、输出、工具结果或编辑证据。

---

## 扫描性能边界

本地扫描不是全盘递归。默认策略是按近窗口、上限和缓存增量处理：

| 边界 | 当前值 |
|------|--------|
| 默认回看窗口 | 30 天 |
| 单次扫描最大窗口 | 30 天 |
| 单次扫描最大文件数 | 5 |
| 单次扫描最大候选数 | 800 |
| 单次扫描最大目录遍历项 | 5000 |
| 每个工具最大文件数 | 200 |
| 每个工具最大候选数 | 800 |
| 每个工具最大目录遍历项 | 5000 |
| 单个 JSON 文件上限 | 16 MiB |
| 单个 JSONL 文件最大行数 | 2000 |
| 单个 CSV 文件最大行数 | 2000 |
| 单个 SQLite 表最大行数 | 2000 |
| 单个 SQLite 文件合计最大行数 | 5000 |
| 单文件最大监控事件数 | 200 |
| 单段采集文本上限 | 4096 字符 |
| 扫描缓存最大行数 | 20000 |
| 监控去重缓存最大行数 | 50000 |
| 本地来源汇总缓存最大行数 | 20000 |

显式 `--since` 和 `--until` 用于小范围回填，超过 30 天的范围会按单次窗口截断。扫描缓存按工具、路径、大小、修改时间和窗口记录，未变化的本地来源会被跳过；缓存和本地来源汇总会保留最近记录，避免本地数据库长期增长。

---

## 常用命令

```bash
# 查看本机工具注册和钩子状态
aitrack status

# 扫描所有默认本地来源，只写入本地账本，不上传
aitrack usage scan

# 只扫描某个工具，并限制回填窗口
aitrack usage scan --tool codex --since 2026-06-01 --until 2026-06-18

# 扫描、汇总并上传用量，同时上传本地会话记录中可还原的监控事件
aitrack usage sync --api-url https://aitrack.example.com --credential <credential>

# 查看本地用量账本状态
aitrack usage status
```

---

## 管理员如何理解结果

- `hooks.<tool> = true` 表示该工具在本机可见或对应钩子可用，不代表一定存在原生编辑钩子。
- `claude`、`codex`、`cursor` 的原生编辑钩子可以生成最完整的文件编辑证据，原生提示词钩子可以补充提示词上下文。
- `usage_basis=native` 和 `usage_basis=local_derived` 都可以进入员工监控统计；上报会保留来源类型，方便审计。
- 默认来源按字段级、本地派生或辅助状态/用量读取处理；每个 source 贡献其真实字段，agent 级数据由多个 source 合并。
- 字段缺失不补假数据。
- 纯用量数据只进入 `/usage/*` 数据面，不会伪装成 `/edits` 监控事件。
- 默认采集不读取完整工作区文件，不记录键盘输入，不向任何第三方服务上传数据。
