# client/src/domain/ — 领域核心

不依赖任何外部框架或 I/O。

## 模块

| 文件 | 职责 |
|------|------|
| `model.rs` | 核心实体（EditRecord、Token 等）|
| `diff.rs` | Myers/LCS diff 精确计算，**禁止**用朴素行数统计替代 |
| `crypto.rs` | HMAC-SHA256 双层签名（record_sig + request_sig） |
| `keywords.rs` | 关键词 SHA-256 指纹，防篡改 |

## 不变式

- `diff.rs`：输出行数必须与 Myers/LCS 精确算法一致，任何修改须保证现有测试通过
- `crypto.rs`：HMAC 计算不得引入 timing side-channel（使用常量时间比对）
- `keywords.rs`：指纹字段变更需同步更新 `CONTRACT.md` 协议版本
