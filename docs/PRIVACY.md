# aitrack 隐私说明 / aitrack Privacy Notice

版本：v1.6 · 最后更新：2026-06-16

---

## 1. 概览 / Overview

本文档说明 aitrack 收集哪些数据、收集原因、存储位置、可见范围，以及您对数据的控制权。

**aitrack 是什么**：一个通用、自托管、开源的员工 AI 编码监控与治理平台。Claude Code、Codex CLI、Cursor 当前具备 native edit hook adapter，可记录这些工具对代码库所做的变更。其他注册 agent 可进入 registry、status、heartbeat 与 local usage source 路线，并通过本地 transcript 扫描补齐 prompt、tool、window 和可还原编辑监控事件。

**本文档的目的**：任何记录开发者行为的工具都应当对其行为保持透明。这不是法律免责声明——而是对"我的数据去哪了？"这一问题的直接回答。

**自托管设计**：aitrack 没有云端组件。部署 aitrack 时，您就是运营方。所有数据均保留在您自己的基础设施内。任何数据都不会发送给 aitrack 项目维护者或任何第三方服务。

---

## 2. 我们收集什么 / What We Collect

每条记录对应 AI 工具触发的一个文件编辑事件。以下是所有收集的字段：

| 数据项 | 收集内容 | 收集原因 | 存储位置 |
|--------|----------|----------|----------|
| **变更差异（diff_hunk）** | 实际新增/删除的代码行，标准 unified diff 格式——仅变更部分，非整个文件 | 分析 AI 实际产出的代码 | 本地 SQLite + 服务端数据库 |
| **文件路径（file_path）** | 相对路径，如 `src/main/java/com/example/Service.java` | 了解哪些模块和层受影响最多 | 本地 SQLite + 服务端数据库 |
| **新增行数（added_lines）** | 本次编辑实际引入的新行数 | 量化 AI 代码贡献 | 本地 SQLite + 服务端数据库 |
| **删除行数（removed_lines）** | 本次编辑实际删除的行数 | 量化 AI 驱动的重构 | 本地 SQLite + 服务端数据库 |
| **时间戳（timestamp）** | Unix 秒，事件发生时刻 | 基于时间的使用分析 | 本地 SQLite + 服务端数据库 |
| **仓库 URL（repo_url）** | Git remote origin URL，如 `git@github.com:org/repo.git` | 按项目分组记录 | 本地 SQLite + 服务端数据库 |
| **分支（branch）** | 当前 Git 分支名 | 区分主干与功能分支 | 本地 SQLite + 服务端数据库 |
| **提交哈希（current_sha）** | 编辑时的 HEAD commit SHA | 将编辑关联到特定代码快照 | 本地 SQLite + 服务端数据库 |
| **主机名（hostname）** | 机器的操作系统 hostname，如 `MacBook-Pro.local` | 在同一凭证跨多台机器使用时识别来源机器；不用于访问控制 | 本地 SQLite + 服务端数据库 |
| **AI 工具类型（tool）** | EditRecord 中来自 native adapter 或本地 transcript 扫描；heartbeat/status 可包含其他注册 agent key | 按工具区分使用模式 | 本地 SQLite + 服务端数据库 |
| **Prompt / tool 监控内容（prompt_summary / metadata）** | 本地 prompt hook 或 transcript 扫描得到的有界 prompt、tool、window 事件内容 | 审计员工 AI 编码使用方式 | 本地 SQLite + 服务端数据库 |
| **Token 标识符（token_key）** | 管理员分配的凭证中的 token 部分，格式 `aitrack_<hex>` | 将记录归因到特定开发者席位 | 仅本地 SQLite（用于本地过滤，不随上传 payload 发送） |
| **设备 ID（device_id）** | 首次运行时生成的 UUIDv4，持久化至本地配置 | 区分使用同一凭证的多台设备 | 本地 SQLite + 服务端数据库 |
| **记录签名（record_sig）** | 绑定以上所有字段的 HMAC-SHA256 签名 | 检测本地记录是否被篡改或伪造 | 本地 SQLite + 服务端数据库 |

**关于 diff_hunk 的说明**：这是变更部分的差异，不是完整文件。如果 AI 修改了一个函数，您得到的是该函数的前后对比——文件中的其他内容不会被包含。差异算法使用 Myers/LCS 最小编辑距离，因此 diff 尽可能小。

**数据域边界**：上表描述的是 `EditRecord` 监控事件域。usage rollup / snapshot 属于标量用量域，可包含请求数、token 数、成本估算或本地客户端活跃统计。token-only 或 usage-only 数据不能伪装成监控事件，也不会生成 diff、行数或 `record_sig` 编辑证据。

**本地用量来源**：aitrack 可从本机日志、JSONL、SQLite、缓存和本地客户端状态中发现用量来源，并在本地状态允许时自动发现凭证或入口。用户不需要手动粘贴第三方 token 才能接入本地用量来源。

---

## 3. 我们不收集什么 / What We Do Not Collect

以下数据**不在收集范围内**，并通过技术手段强制保证：

| 不收集的数据 | 技术保障方式 |
|-------------|-------------|
| **完整文件内容** | 仅存储 `diff_hunk`（变更部分）。捕获流程不读取或存储工具钩子 payload 之外的任何内容。 |
| **完整 AI 响应内容** | usage / transcript 扫描面只提取 prompt、tool、window、编辑事件和 token bucket、message count、source cost，不把完整 assistant response 作为独立数据面上传。 |
| **密码、私钥、证书** | 捕获流程包含文件路径合理性检查，自动跳过匹配以下模式的文件：`*.key`、`*.pem`、`*.pfx`、`*.p12`、`*.env`、`*secret*`、`*password*` 等敏感文件名。这些路径不会生成任何记录。 |
| **完整 AI 对话历史** | 本地 hook / transcript 扫描只抽取 prompt、tool、window、编辑事件和 token bucket、message count、source cost 字段，不把完整对话历史作为独立数据面上传。 |
| **凭证中的 HMAC secret 部分** | 凭证由 `<token>-<hmac_secret>` 组成。`hmac_secret` 仅在本地用于计算签名，从不通过网络发送。 |
| **与编码无关的个人身份信息** | aitrack 不访问开发环境以外的任何系统。 |

---

## 4. 数据如何存储 / How Data Is Stored

### 本地存储（客户端侧）/ Local storage (client side)

- 目录：`~/.aitrack/`
- 数据库：`~/.aitrack/records.db`（SQLite）
  - 文件权限：0600——仅文件所有者可读
  - `EditRecord` 在上传前写入此处；可通过 `aitrack inspect` 查看
- 配置文件：`~/.aitrack/config.toml`
  - 文件权限：0600
  - 包含：API URL、凭证（token + hmac_secret 合并值）、设备 ID

### 服务端存储 / Server-side storage

- 数据库：PostgreSQL / ParadeDB（Java 本地开发可用 H2，Go 服务端生产必须使用 PostgreSQL）
- 仅有直接数据库访问权限的管理员可查询原始记录
- 所有数据保留在您自己的基础设施内——不涉及外部服务

### 加密 / Encryption

- 每个凭证中的 `hmac_secret` 部分在服务端使用 AES-256-GCM 加密存储。即使拥有数据库访问权限的管理员也无法以明文读取它。
- Token 在服务端以 SHA-256 哈希值存储。原始凭证仅在签发时返回一次。
- 传输中的记录受双层 HMAC-SHA256 签名保护（per-record `record_sig` + per-request `X-AiTrack-Signature`），任何传输中的篡改均可被检测。

---

## 5. 谁可以访问数据 / Who Can Access the Data

| 角色 | 可见数据 | 访问方式 |
|------|----------|----------|
| **您（开发者）** | 您机器上生成的所有记录，包括 diff 内容 | `aitrack inspect --limit 100` — 直接读取本地 SQLite，无需网络 |
| **管理员** | 所有开发者的所有记录（按 token 归因） | 服务端数据库或管理员 API（需要 `X-Admin-Key`） |
| **其他团队成员（非管理员）** | 无法直接访问服务端记录 | — |
| **第三方** | 无 | — |

**关于第三方访问**：由于 aitrack 是自托管的，所有数据传输均在您自己的网络内进行。aitrack 不调用任何外部 API，也不向 aitrack 项目维护者或任何供应商发送数据。如果您部署在云虚拟机上，云服务商的标准条款适用于基础设施，但 aitrack 本身不会将数据路由至外部。

---

## 6. 数据保留 / Data Retention

**当前版本没有自动过期机制。** 上传到服务端的记录会一直保留，直到显式删除。

清理方法：

- **本地记录**：`aitrack clean --all` 从本地 SQLite 数据库中删除已同步的记录。
- **服务端记录**：管理员可通过直接数据库操作或管理员 API，按 token key、时间范围或仓库删除记录。

可配置的服务端 TTL 计划在未来版本中提供。届时本文档将相应更新。

---

## 7. 自托管用户的权利 / Your Rights as a Self-Hosted User

由于您自托管 aitrack，您掌控一切：

**查看本地数据 / Inspect your local data：**
```bash
aitrack inspect --limit 100      # 查看最近 100 条记录（含 diff 内容）
aitrack inspect --pending        # 查看尚未上传的记录
aitrack stats                    # 按工具和仓库分组的聚合统计
aitrack status                   # 检查已安装的工具钩子
```

**随时移除钩子 / Remove hooks at any time：**
```bash
aitrack remove --claude          # 移除 Claude Code 钩子
aitrack remove --codex           # 移除 Codex CLI 钩子
aitrack remove --cursor          # 移除 Cursor 钩子
```

钩子移除后，该工具不再创建新记录。

**删除数据 / Delete your data：** 作为自托管运营方，您对本地 SQLite 文件（`~/.aitrack/records.db`）和服务端数据库均有完全访问权限。您可以直接删除记录，或请管理员执行。没有锁定，也没有数据保留在您的基础设施之外。

**完全停止采集 / Stop collection entirely：** 卸载已安装钩子（`aitrack remove --claude --codex --cursor`，或对注册工具使用 `aitrack remove --tool <name>`）或删除 aitrack 二进制文件。数据库中已有的记录不受影响，需手动删除。

---

## 8. 关于 Prompt 数据的说明 / A Note on Prompt Data

`prompt_summary` 由本地 prompt hook 或 transcript 扫描产生。客户端会保存有界 prompt 内容，并将其作为可选字段附加到编辑记录，用于审计和画像维度。

usage rollup / snapshot 仍是独立标量域，用于 token bucket、message count、source cost、quota 和 subscription 状态。它不替代 `EditRecord`，也不会伪造 diff、行数或 `record_sig` 编辑证据。

---

## 9. 安全机制 / Security Mechanisms

**双层 HMAC-SHA256 签名 / Dual HMAC-SHA256 signatures：**

- Per-record 签名（`record_sig`）：记录写入本地数据库时计算。它绑定 device_id、hostname、timestamp、tool、file_path、repo_url、commit SHA、行数以及 diff 的哈希值。服务端拒绝签名无效的记录。
- Per-request 签名（`X-AiTrack-Signature`）：覆盖整个上传请求体和时间戳。防止重放攻击和传输中的篡改。

**凭证存储 / Credential storage：**

- `hmac_secret` 在服务端使用 AES-256-GCM 加密存储。
- Token 以 SHA-256 哈希值存储。明文凭证仅在签发时返回一次，之后无法从服务端恢复。

**路径过滤 / Path filtering：** 捕获流程（10 步中的第 8 步）检查每个文件路径的合理性。匹配敏感文件名模式的文件会被自动跳过——不会写入记录，也不会计算 diff。

**限流 / Rate limiting：** 服务端对每个（token, file_path）对每小时限制 30 条记录，防止通过编辑次数虚增来刷数据。

**心跳 / Heartbeat：** 客户端定期向服务端上报 native hook 与注册 agent 状态，让管理员可以检测钩子被移除或本地 agent 状态异常（强化点 H3）。

**本地数据库权限 / Local database permissions：** `~/.aitrack/records.db` 和 `~/.aitrack/config.toml` 均以 0600 权限创建。在多用户机器上，其他操作系统用户无法读取这些文件。

---

## 10. 联系与反馈 / Contact and Feedback

如果您对数据处理方式有疑问、发现安全问题，或希望对本文档提出改进建议：

- 在 aitrack GitHub 仓库提 issue
- 或直接联系仓库维护者

对于安全漏洞，请遵循仓库根目录 `SECURITY.md` 中描述的负责任披露流程。

---

*本文档随软件版本一同维护。如果采集范围发生变更，本文档将在版本发布前更新。*
