# 贡献指南

[English](#english)

## 中文

欢迎为 aitrack 提交改进。

### 基本原则

- 改协议字段（`CONTRACT.md`）必须同时更新三端：Rust 客户端、Java 服务端、Go 服务端
- 新改动必须通过测试并满足覆盖率门槛
- 不提交密钥、token、环境文件或私有路径

### 环境要求

| 组件 | 最低版本 |
|------|---------|
| Rust | stable（推荐最新 stable） |
| JDK | 17 |
| Maven | 3.8+ |
| Go | 1.22+ |
| Docker | 用于端到端集成验证 |

### 构建与测试

**Rust 客户端**

```bash
cd client
cargo build --release
cargo test
# 覆盖率（需先安装 cargo-llvm-cov）
cargo llvm-cov --summary-only
```

**Java 服务端**

```bash
cd server-java
# 单测 + 覆盖率检查（LINE ≥ 90% 才通过）
mvn verify
# 仅跑单测
mvn test
```

**Go 服务端**

```bash
cd server-go
go test ./...
```

### 覆盖率门槛

| 组件 | 工具 | 门槛 |
|------|------|------|
| Rust 客户端 | cargo-llvm-cov | 函数覆盖率 ≥ 90% |
| Java 服务端 | JaCoCo | LINE 覆盖率 ≥ 90%（`mvn verify` 强制） |

提交前必须确保覆盖率门槛通过，不允许带失败或警告的测试合入。

### 分支规范

- 主干：`main`
- 功能分支：`feat/<short-description>`
- 修复分支：`fix/<short-description>`
- 协议变更：`contract/<version>-<description>`（需注明涉及哪些端）

### Commit 规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/v1.0.0/)：

```
<type>(<scope>): <subject>

[body]
[footer]
```

常用 type：`feat` / `fix` / `refactor` / `test` / `docs` / `chore`

scope 示例：`client` / `server-java` / `server-go` / `contract` / `testkit`

### Pull Request 规范

- PR 描述说明改动范围和测试结论
- 涉及协议变更时，列出三端同步状态
- 不得绕过 CI 检查强制合并

## English

Contributions to aitrack are welcome.

### Principles

- Protocol field changes (`CONTRACT.md`) must be applied to all three components simultaneously: Rust client, Java server, Go server
- All changes must pass tests and meet coverage thresholds
- Do not commit secrets, tokens, environment files, or private filesystem paths

### Requirements

| Component | Minimum version |
|-----------|----------------|
| Rust | stable (latest stable recommended) |
| JDK | 17 |
| Maven | 3.8+ |
| Go | 1.22+ |
| Docker | for end-to-end integration verification |

### Build and test

**Rust client**

```bash
cd client
cargo build --release
cargo test
# Coverage (requires cargo-llvm-cov)
cargo llvm-cov --summary-only
```

**Java server**

```bash
cd server-java
# Unit tests + coverage check (LINE >= 90% required)
mvn verify
# Tests only
mvn test
```

**Go server**

```bash
cd server-go
go test ./...
```

### Coverage thresholds

| Component | Tool | Threshold |
|-----------|------|-----------|
| Rust client | cargo-llvm-cov | function coverage >= 90% |
| Java server | JaCoCo | LINE coverage >= 90% (enforced by `mvn verify`) |

Tests must pass with zero failures or warnings before submitting.

### Branch naming

- Main: `main`
- Feature: `feat/<short-description>`
- Fix: `fix/<short-description>`
- Protocol change: `contract/<version>-<description>` (note which components are affected)

### Commit convention

Follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

Common types: `feat` / `fix` / `refactor` / `test` / `docs` / `chore`

Scope examples: `client` / `server-java` / `server-go` / `contract` / `testkit`

### Pull request requirements

- Describe the change scope and test results in the PR description
- For protocol changes, list the sync status across all three components
- Do not force-merge by bypassing CI checks
