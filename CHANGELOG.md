# 更新日志

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。

---

## v1.1.0 — 2026-05-17

### 发布说明

v1.1.0 是协议 v1.1 对应的正式版本，核心变更是在上报记录和心跳中引入 `hostname` 字段，使同一 token 在多台机器上的编辑活动可被逐机器追溯。

### 新增

- `hostname` 字段加入 `records` 表 schema 及上报 JSON 结构（`CONTRACT.md` §Upload Request）。
- `record_sig` HMAC 计算绑定 `hostname`，防止跨设备伪造（hardening point 12.1/12.2）。
- 心跳请求中携带 `hostname`，服务端 `devices` 接口可见每台机器的心跳状态。
- Rust 客户端 `capture` 流程第 6 步：从 OS 读取 hostname 并写入本地 SQLite 和上报体。
- Java 服务端 `ValidationService` 新增 hostname 存储与设备去重逻辑。
- Go 服务端同步支持 hostname 字段解析和存储。

### 变更

- `CONTRACT.md` 升版至 v1.1，新增 `v1.1 change` 说明段落，字段顺序文档更新。
- 服务端 `record_sig` 校验规范字符串加入 `hostname` 行（`CONTRACT.md` §Record Signature）。
- `HmacSecretEncryptorTest`、`SignatureServiceCanonicalTest` 同步更新预期值。

### 技术说明

`hostname` 是透明可见性机制，不作为访问控制手段；同一 token 多机使用属正常场景，管理员可通过 `/api/v1/ai-track/devices` 逐机审查。

---

## v1.0.0 — 2026-05-01

### 发布说明

v1.0.0 是 aitrack 的初始正式版本，建立了 Rust 客户端 + Java 服务端的双组件架构，以及完整的加固校验链（hardening points 12.1–12.8）。

### 新增

- Rust CLI 客户端，支持 `init / remove / capture / inspect / stats / status / clean / heartbeat` 全套命令。
- 支持 Claude Code、Codex CLI、Cursor 三种 AI 编码工具的 hook 安装与卸载，操作幂等。
- 本地 SQLite 存储（`~/.aitrack/records.db`，权限 0600），`config.toml`（权限 0600）持久化配置与 `device_id`。
- Myers/LCS 真差分算法（`similar` crate），防止朴素行数统计被刷高（hardening point 12.4）。
- `record_sig` HMAC-SHA256 签名，绑定 `token_key + device_id + timestamp + tool + file_path + repo_url + current_sha + added_lines + removed_lines + sha256(diff_hunk)`，防止本地记录篡改（hardening point 12.1/12.2）。
- 请求级 HMAC 签名（`X-AiTrack-Signature`），防止重放攻击，时间窗口 300 秒（hardening point 12.2）。
- 心跳机制：每次 `capture` 结束后节流发送，1 小时内最多一次；`aitrack heartbeat` 可强制发送（hardening point 12.3）。
- 适配器解析失败写本地日志，不静默吞错（hardening point 12.8）。
- Java 服务端（Spring Boot 3 / JDK 17 / H2 或 PostgreSQL），10 步校验链，覆盖签名、重放、差分一致性、仓库白名单、路径合理性、行数上限、速率限制。
- `AES-256-GCM` 加密存储 `hmac_secret`（`HmacSecretEncryptor`）。
- 服务端 testkit 工厂模式（`EditDtoFactory`、`TamperedFactory` 等），JaCoCo 覆盖率门槛 ≥ 90%。
- Rust 客户端测试覆盖率：行 87.75%，函数 90.24%，含 `testkit/factories.rs` 种子确定性构建器。
- `CONTRACT.md` 作为客户端与服务端的协议单一可信来源（Single Source of Truth）。

### 技术说明

初始版本以单机自托管为主要场景；H2 内存/文件数据库开箱即用，生产环境可切换 PostgreSQL（`application.yml` 配置切换，无需修改业务代码）。
