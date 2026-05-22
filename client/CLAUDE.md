# client/ — Rust CLI

## 概览

`aitrack` 二进制。六边形架构：`domain/port/adapter` 三层。

## 构建

```bash
cargo build --release                     # 本机
cargo zigbuild --release --target x86_64-unknown-linux-musl   # musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
```

### musl CFLAGS（必须，缺少则 sqlite-vec C 编译失败）

```bash
export CFLAGS_x86_64_unknown_linux_musl="-Du_int8_t=uint8_t -Du_int16_t=uint16_t -Du_int32_t=uint32_t -Du_int64_t=uint64_t"
export CFLAGS_aarch64_unknown_linux_musl="-Du_int8_t=uint8_t -Du_int16_t=uint16_t -Du_int32_t=uint32_t -Du_int64_t=uint64_t"
```

### 六平台 release 目标

| Target | Binary 后缀 |
|--------|------------|
| `x86_64-apple-darwin` | — |
| `aarch64-apple-darwin` | — |
| `x86_64-unknown-linux-musl` | — |
| `aarch64-unknown-linux-musl` | — |
| `x86_64-pc-windows-msvc` | `.exe` |
| `aarch64-pc-windows-msvc` | `.exe` |

## 关键约束

- `reqwest`：`default-features=false`，使用 `rustls-tls`（禁止 OpenSSL 依赖）
- `StoragePort` 方法签名含 `rusqlite::Result`（已知基础设施泄漏，中优先级待办）
- `HttpUploader::upload_batch` 通过 `UploadPort` 接口实现真实 HTTP POST

## aitrack update 子命令

- 签名验证：ed25519（pubkey 硬编码在 `src/update.rs`，**不得修改**）
- 流程：GitHub Releases API → 下载 binary + .sig → 验证 → 原子替换
- 全零密钥启动断言防误发布
- Windows 原子替换暂未处理（低优先级）

## 测试

```bash
cargo test
```

覆盖率目标 ≥ 90%（当前 90.71%，291 tests）。
