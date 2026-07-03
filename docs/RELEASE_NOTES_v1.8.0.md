# v1.8.0 发布说明

## 概览

v1.8.0 将 aitrack 的本地采集能力扩展到 35 个规范工具 key，并把每个工具的真实本地结构合并成 agent 级完整数据面。

本版本继续保持 `EditRecord` 监控事件和 `/usage/*` 标量用量数据面分离：可还原的提示词、助手输出、工具调用、工具结果、窗口和编辑线索进入监控事件；token、消息数、成本、额度和订阅信息进入用量汇总或快照。

## 主要变化

- 默认本地扫描覆盖 35 个规范工具 key。
- 每个默认工具 key 都必须有本地来源矩阵、fixture、parser 路径和字段级断言。
- 本地来源按字段级原生读取、本地派生读取和辅助状态/用量来源分层处理，再合并为 agent 级完整数据面。
- 用量上报保留 `usage_basis=native` 与 `usage_basis=local_derived`，用于区分来源计量和本地派生计量。
- 客户端上传批次、usage outbox、扫描窗口、扫描候选、目录遍历、文件读取、SQLite 行数、zstd 解压和 sidecar 文件数都有硬上限。
- Java 与 Go 服务端对 usage rollup 做幂等聚合 upsert，不保存提示词、助手输出或工具结果原文。
- Java 与 Go 服务端对旧的编辑监控原文字段执行 retention 清洗，保留签名、标量、时间和索引信息。

## 默认本地扫描工具

默认执行 `aitrack usage scan` 或 `aitrack usage sync` 且未指定 `--tool` 时，会扫描以下 35 个规范 key：

`claude`、`codex`、`cursor`、`trae`、`qwen`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode`。

显式指定 `--tool` 时，也接受 `roocode`、`kilo-code`、`gajae-code` 作为别名，并分别归并到 `roo-code`、`kilocode`、`gjc`。

## 升级说明

- 本版本没有移除既有服务端写入 API。
- `/usage/rollup` 的唯一键包含 `usage_basis`，同一工具、模型、账号下的来源计量和本地派生计量会分开聚合。
- 默认扫描仍是增量、有窗口、有缓存的本地扫描；历史回填请显式使用 `--since` / `--until`。
- 字段缺失不会被补成假数据，也不会用 token、成本或请求数反推出提示词、输出、工具结果或编辑证据。

## 验证记录

- 架构门禁通过。
- 本地扫描矩阵自检通过：67 个 source entry。
- PR CI 门禁覆盖 Rust、Java、Go、架构、覆盖率、Java + Go E2E、Rust 本地扫描 E2E、Codecov、FOSSA 和自动化审查。
