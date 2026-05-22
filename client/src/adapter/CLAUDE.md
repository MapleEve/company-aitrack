# client/src/adapter/ — 适配器层

实现 `port/` 定义的接口。三个子目录各负责一个外部边界。

## 子目录

| 目录 | 接口 | 外部依赖 |
|------|------|---------|
| `event/` | 钩子事件捕获（Claude Code / Codex / Cursor） | OS 文件系统事件 |
| `http/` | `UploadPort`：`HttpUploader::upload_batch` | reqwest（rustls-tls） |
| `sqlite/` | `StoragePort`：本地 SQLite 存储 | rusqlite |

## 已知问题

`sqlite/` 中 `StoragePort` 方法签名含 `rusqlite::Result` — 基础设施类型泄漏到 port 层。中优先级待修，修改时需同步更新 `port/` 接口定义。

## 约束

- `http/` 的 reqwest 配置：`default-features=false, features=["rustls-tls"]`，禁止引入 OpenSSL
- 新增适配器必须通过 `port/` 接口，不得绕过 port 直接被 domain 调用
