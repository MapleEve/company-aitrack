# AiTrack 产品路线图

**版本**：v1.7 draft
**更新日期**：2026-06-17
**当前状态**：v1.7.0 待发布；v1.7 后续强化、v1.8（2026 Q4）与 v2.0（2027 Q1）规划中

---

## 项目定位

AiTrack 是一款通用、自托管、开源的 **员工 AI 编码监控与治理工具**。团队将 AiTrack 部署在自有基础设施上，对开发者使用 AI 编码工具的行为进行可信采集、签名验证与数据分析，全程数据留在部署方控制的环境内。

---

## 愿景

从基础的「可信数据采集」出发，逐步演进为「语义洞察 + 自适应改进」的完整闭环，再扩展至「跨 AI 工具、跨后端语言、跨管理形态」的统一治理平台。

让团队不仅能统计 AI 工具用了多少，还能理解怎么用、用在哪里最有价值；同时，让 IT 管理者对客户端采集链路的完整性和可信度拥有可观测能力。

---

## 已交付 / 待发布版本（v1.0.0 – v1.7.0）

### 核心功能概览

| 版本 | 主要能力 | 状态 |
|------|----------|------|
| v1.0–v1.2 | Rust CLI 客户端；Claude Code / Codex CLI / Cursor 钩子管理；Myers/LCS 精确 diff；HMAC-SHA256 双层签名；Java + Go 双服务端；10 步服务端校验链；心跳机制；统计查询 API；Docker 一键部署；CI 覆盖率 ≥ 90% | 已交付 |
| v1.3 | 服务端数据库升级为 ParadeDB（PostgreSQL + pg_search + pgvector）；向量列与全文索引基础设施；全文检索 API（BM25）；向量 ANN 检索 API；客户端 sqlite-vec 本地嵌入存储 | 已交付 |
| v1.4 | 开发者 AI 工具使用画像（使用频率 / 使用深度 / 语言分布）；每日聚合 Job；Java + Go 等价实现 | 已交付 |
| v1.5 | prompt 捕获前置：UserPromptSubmit hook + prompt_summary 字段 + prompt_patterns 画像维度 | 已交付 |
| v1.6.0 | 六边形架构重构（domain / port / adapter）；`aitrack update` 子命令（ed25519 签名验证）；关键词完整性指纹（SHA-256）；testapp 端到端真实链路 | 已交付 |
| v1.6.1 | Go 服务端完全迁移为 PostgreSQL-only（移除 SQLite 回退）；E2E 竞态修复；CI 改用原生工具链（llvm-cov / mvn verify）替代 Docker 构建验证 | 已交付 |
| v1.7.0 | 动态 agent registry/status/heartbeat；37 个默认 local-source agent；usage rollup / subscription snapshot 数据面；Java + Go usage API；本地 transcript 监控恢复；30 天默认窗口、按需小回填和文件游标缓存；local-source E2E 与架构门禁 | 待发布 |

### 当前 agent 支持边界

- Claude Code、Codex CLI、Cursor：已具备 native edit hook adapter，可生成 `EditRecord` 编辑证据。
- Claude Code：额外支持 native prompt hook；Codex CLI 与 Claude Code 可从本地状态提取 quota / subscription snapshot。
- 动态 agent registry/status/heartbeat：心跳 `hooks` 已按 agent key 动态表达，支持登记更多 agent 的安装状态。
- 默认本地扫描覆盖 `claude`、`codex`、`cursor`、`trae`、`qwen`、`baidu-comate`、`wenxin`、`antigravity`、`opencode`、`qoder`、`qoder-cn`、`qoder-work`、`qoder-work-cn`、`wukong`、`hermes`、`openclaw`、`gemini`、`copilot`、`cline`、`roo-code`、`kiro`、`zed`、`goose`、`amp`、`droid`、`pi`、`mux`、`crush`、`codebuff`、`kilo`、`kilocode`、`kimi`、`gjc`、`grok`、`synthetic`、`warp`、`zcode`；显式 `--tool` 也接受 `roocode`、`kilo-code`、`gajae-code` 作为别名。
- local usage source：已支持按本机日志、JSONL、NDJSON、SQLite、CSV、缓存和本地客户端状态接入 usage rollup / snapshot，按 token bucket、message count 和 source cost 聚合；同时可将 transcript 中的 prompt、tool、window 和可还原编辑监控事件补入 `EditRecord` 上报链路。
- 扫描性能边界：默认 30 天近窗口增量扫描；显式 `--since/--until` 做小范围回填；每个 agent 有候选、文件数、目录遍历和行数上限；本地文件游标缓存可跳过未变化来源。

### 当前成功指标（v1.7.0 待发布基线）

- 捕获成功率 ≥ 99%
- 误拒率 < 0.1%
- 服务端校验链吞吐量 ≥ 500 rps
- 心跳检测延迟 ≤ 1 小时
- 三组件（client / server-java / server-go）CI 覆盖率均 ≥ 90%
- Rust 客户端单测 300 通过
- local-source E2E 矩阵覆盖 37 / 37；PR 门禁要求覆盖率不低于 90%
- 未变化本地来源的第二次 `usage sync` 必须解析 0 条 message 和 0 条 monitoring event

---

## 规划中：v1.7 后续强化（2026 Q3）

**主题**：可信采集端 + 生产级部署强化

**目标**：在 v1.7.0 local-source 基线之上，让企业管理者对「native hook / 注册 agent 是否在线、二进制是否被替换」拥有主动可观测能力；同时将大型企业（500+ 开发者）的部署与运行门槛降至生产可用水平。

| 里程碑 | 功能 | 说明 |
|--------|------|------|
| M8 | 逆向心跳（Reverse Heartbeat） | 服务端主动检测客户端钩子健康状态；超时未响应设备标记为 `unreachable`，供管理员审查 |
| M9 | 并发加固（Concurrency Hardening） | 500+ 开发者并发上报场景下的批量入库稳定性；P99 延迟目标 < 500ms；连接池与线程池全面调优 |
| M10 | DockerHub 官方镜像 + Docker Compose 一键部署 | `docker-compose up` 单命令启动全套服务；java / go 双服务端镜像带版本标签；环境变量文档完整 |
| M11 | 本地防篡改：二进制完整性验证 | 启动时 ed25519 自检；关键词库由明文指纹升级为加密存储 + 启动时校验；检测到篡改时上报 tamper_alert |
| M12 | Local source diagnostics | `aitrack usage sources` / dry-run 输出发现到的来源、窗口、缓存命中、跳过原因和预计扫描量，方便管理员评估本机开销 |
| M13 | 数据治理策略 | prompt / transcript 字段 allowlist、脱敏规则、保留期和 per-agent 采集开关，降低敏感数据落库风险 |

预计工期：8 周

---

## 规划中：v1.8（2026 Q4）

**主题**：从治理基线工具升级为跨后端、跨 AI 工具、跨管理形态的治理平台

**目标**：在保持现有 Java / Go 双服务端的基础上，新增第三实现（Rust）、扩展 AI 工具覆盖、引入服务端 Skills 执行能力，并通过 CLI 与 MCP 两种新管理形态覆盖 DevOps 与 AI 原生工作流。

| 里程碑 | 功能 | 说明 |
|--------|------|------|
| M14 | Rust 服务端 | 基于 axum + sqlx + tokio；与 Java / Go 协议完全等价；目标资源占用 ≤ 32MB 空载；适用于边缘部署与资源受限场景 |
| M15 | Native edit adapter 扩展 | 按 agent 自身本地编辑事件能力逐个落地 native edit adapter，提升文件编辑类 `EditRecord` 的 diff 与行数精度 |
| M16 | Agent-specific source packs | 在通用 JSON/SQLite/CSV 解析之外，逐个补充 agent 专属字段映射、fixture 和回归测试，提高 prompt、tool、window、edit 和 cost 字段的提取精度 |
| M17 | 服务端 Skills + 服务端 CLI（纯 Rust） | Skills：服务端沙箱执行能力（初始内置 summarize_edits / detect_pattern / suggest_refactor）；CLI：管理员命令行工具，无需 JVM / Go 运行时，支持 token 管理、设备查询、统计与画像查询 |
| M18 | MCP 管理接口 | 将 aitrack 服务端暴露为 MCP Server；管理者可在 Claude Desktop / Claude Code 中直接查询统计、设备、画像、相似代码等数据，无需传统后台 UI |

预计工期：12 周

完成后，agent 覆盖从「动态注册 + 状态心跳 + 通用本地用量来源」扩展为「按能力落地编辑适配 + 专属 source pack」，管理形态从「Web 后台 + REST API」扩展至「CLI + MCP」。

---

## 规划中：Phase 4 / v2.0（2027 Q1）

**主题**：eval 反馈进化闭环

**目标**：在 prompt 捕获基础设施（v1.5 已落地）之上，构建「prompt 捕获 → 自动 eval 评判 → skill 进化」的全自动闭环。

核心能力（规划）：
- 自动 eval 评判引擎，识别负面与模糊信号
- 规则自适应机制
- 闭环可视化仪表板

---

## 路线图总览

```
2026 Q2（已交付）       2026 Q3（v1.7）        2026 Q4（v1.8）         2027 Q1（v2.0）
──────────────────────────────────────────────────────────────────────────────────────
可信采集基线（v1.2）    动态 agent registry      Rust 服务端（M14）       eval 反馈闭环
Myers 精确 diff        local usage source       Native adapter 扩展      自动评判引擎
HMAC 双层签名          逆向心跳（M8）            Agent source packs       规则自适应
Java / Go 双服务端     并发加固（M9）            服务端 Skills（M17）     闭环仪表板
10 步校验链            DockerHub + Compose       Rust 服务端 CLI（M17）
向量化基础（DB-1/2）                            MCP 管理接口（M18）
语义检索 API（BM25/ANN）
开发者使用画像（v1.4）
prompt 捕获（v1.5）
六边形架构（v1.6）
bounded local scan（v1.7）
```

---

## 能力矩阵

| 能力 | v1.6 | v1.7（待发布 / Q3） | v1.8（Q4） | v2.0（2027 Q1） |
|------|:-----------:|:----------:|:----------:|:---------------:|
| 精确行数统计（Myers diff） | ✓ | ✓ | ✓ | ✓ |
| HMAC-SHA256 双层签名 | ✓ | ✓ | ✓ | ✓ |
| 10 步服务端校验链 | ✓ | ✓ | ✓ | ✓ |
| Java + Go 双服务端 | ✓ | ✓ | ✓ | ✓ |
| 结构化统计查询 API | ✓ | ✓ | ✓ | ✓ |
| Docker 自托管部署 | ✓ | ✓ | ✓ | ✓ |
| 向量化存储（本地 + 服务端） | ✓ | ✓ | ✓ | ✓ |
| 全文检索（BM25） | ✓ | ✓ | ✓ | ✓ |
| 向量 ANN 检索 | ✓ | ✓ | ✓ | ✓ |
| 开发者使用画像 | ✓ | ✓ | ✓ | ✓ |
| prompt / transcript 监控 | ✓ | ✓ | ✓ | ✓ |
| 六边形架构 | ✓ | ✓ | ✓ | ✓ |
| ed25519 自更新验证 | ✓ | ✓ | ✓ | ✓ |
| 动态 agent registry / status / heartbeat | 部分 | ✓ | ✓ | ✓ |
| local usage source / usage rollup | — | ✓ | ✓ | ✓ |
| bounded local scan / 文件游标缓存 | — | ✓ | ✓ | ✓ |
| 逆向心跳（server → client） | — | 规划中 | ✓ | ✓ |
| 500+ 并发加固 | — | 规划中 | ✓ | ✓ |
| DockerHub 官方镜像 + Compose | — | 规划中 | ✓ | ✓ |
| 本地防篡改 / 二进制完整性 | — | 规划中 | ✓ | ✓ |
| local source diagnostics / dry-run | — | 规划中 | ✓ | ✓ |
| prompt / transcript 数据治理策略 | — | 规划中 | ✓ | ✓ |
| Rust 服务端（第三实现） | — | — | 规划中 | ✓ |
| 更多 native edit adapter | — | — | 规划中 | ✓ |
| agent-specific source packs | — | — | 规划中 | ✓ |
| 服务端 Skills | — | — | 规划中 | ✓ |
| 服务端 CLI（纯 Rust） | — | — | 规划中 | ✓ |
| MCP 管理接口 | — | — | 规划中 | ✓ |
| eval 反馈进化闭环 | — | — | — | 规划中 |

---

## 部署说明

AiTrack 仅提供私有化自托管部署，不提供 SaaS 服务。

- **当前（v1.7 待发布）**：通过 Docker 镜像自行构建部署；Java / Go 服务端与 Rust 客户端均由 CI 覆盖率和 E2E 门禁验证
- **v1.7 后续起**：DockerHub 提供官方预构建镜像，`docker-compose up` 即可完成全套部署
- **数据主权**：所有采集数据存储在企业自有数据库中，不上报任何外部服务

---

*路线图内容反映当前规划，具体排期可能随产品实际进展调整。*
