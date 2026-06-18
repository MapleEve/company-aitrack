# 安全策略

## 适用对象

这篇面向 aitrack 自托管用户、部署者和漏洞报告者。aitrack 是一个在开发者本地机器和自托管服务端之间传输 AI 编辑遥测数据的系统，请把它当成会处理 git 仓库元数据、代码差分内容、API token 和 HMAC 密钥的基础设施组件。

## 支持版本

| 版本 | 支持状态 |
|------|---------|
| `v1.7.x`（当前） | 支持安全修复 |
| `v1.6.x` 及更早版本 | 不承诺安全修复 |

## aitrack 安全模型摘要

### 客户端侧

- **本地存储权限**：`~/.aitrack/config.toml`、`~/.aitrack/records.db` 和本地用量账本均应使用仅当前用户可读写的权限，避免同机其他用户读取；写入过程先写临时文件再 rename，避免中断留下权限不当的半成品。
- **record_sig HMAC**：每条记录在写入本地 SQLite 前计算 HMAC-SHA256 签名，绑定 `token_key + device_id + hostname + timestamp + tool + file_path + repo_url + current_sha + added_lines + removed_lines + sha256(diff_hunk)`，防止本地记录被篡改后重传（加固点 H1/H2）。
- **请求级签名**：上传请求携带 `X-AiTrack-Signature`（HMAC 覆盖时间戳和请求体哈希），服务端校验 300 秒时间窗口，防止重放攻击。
- **Myers/LCS 真差分**：使用 `similar` crate 计算最小差分，防止朴素行数统计被刷高（加固点 H4）。
- **心跳**：定期上报 hook 安装状态，服务端可发现 hook 被静默卸载（加固点 H3）。
- **解析失败记录日志**：适配器解析失败写 stderr，不静默吞错（加固点 H6）。

### 服务端侧

- **Token 存储**：服务端只存储 `sha256(token)`，明文 token 仅在签发时展示一次。
- **HMAC secret 加密存储**：`hmac_secret` 以 AES-256-GCM 加密存储（`HmacSecretEncryptor`）；因需要重算 `record_sig`，必须可解密，代码中有注释说明此取舍。
- **10 步校验链**：见 `server-java/README.md` §Hardening Points，覆盖签名验证、重放防护、差分一致性、仓库白名单、路径合理性、行数上限、速率限制。
- **Flagged 机制**：可疑记录仍会被摄入但标记为 flagged，便于管理员事后审查，不影响正常上报。

## 威胁模型

aitrack 的主要风险来自：

- `config.toml` 中的 `credential`（包含 token 和 hmac_secret）泄露。
- 本地 `records.db` 文件权限过宽被同机其他进程读取。
- 本地用量账本、会话记录或缓存中包含敏感提示词、路径、窗口标题或代码摘要。
- 服务端 `/admin/tokens` 接口暴露到不可信网络。
- 上报数据中包含敏感 `diff_hunk` 内容（代码差分包含机密信息）。

## 部署侧必须做的硬化

- 为 `hmac_secret` 生成强随机值（至少 32 字节熵）。
- 生产环境务必用网络 ACL 或 admin secret 头保护 `/admin/**` 接口。
- `application.yml` 中的 `aitrack.encryption-key` 使用强随机值，不使用默认值。
- 开启 `aitrack.repo-whitelist.enforce=true` 可拒绝未知仓库的上报。
- 定期轮换 credential（通过重新签发 token 实现）。

## 公开报告边界

公开 Issue / PR / 讨论中不要包含：

- 任何 `credential`、`X-AiTrack-Signature` 值（credential 包含 token 和 hmac_secret）。
- 完整 `config.toml` 文件内容。
- 含有真实代码内容的 `diff_hunk`。
- 本地私有路径或仓库 URL。
- 未脱敏的本地会话记录、用量扫描文件、窗口标题或提示词正文。

可以提供：

- aitrack 版本号（`aitrack --version`）。
- 操作步骤和预期与实际结果。
- 脱敏后的错误输出（隐去 token、secret、路径中的用户名）。

## 漏洞上报

如果发现未修复的安全问题，请使用 **GitHub Security Advisory** 或维护者指定的安全联系渠道。不要在公开 Issue 中披露可利用细节。

## 相关文档

- [协议契约](./CONTRACT.md)
- [客户端说明](./client/README.md)
- [Java 服务端说明](./server-java/README.md)
