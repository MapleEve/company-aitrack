# CLAUDE.md — company-aitrack

> Claude Code 执行规则。各子目录有对应 CLAUDE.md，进入子目录时自动加载。

## 仓库结构

| 路径 | 内容 | 详细规则 |
|------|------|---------|
| `client/` | Rust CLI（`aitrack` 二进制） | `client/CLAUDE.md` |
| `server-java/` | Java Spring Boot 服务端（主推） | `server-java/CLAUDE.md` |
| `server-go/` | Go chi 服务端（等价备选） | `server-go/CLAUDE.md` |
| `e2e/` | E2E 集成测试 | `e2e/CLAUDE.md` |
| `docker/` | Dockerfile × 3 + compose | `docker/CLAUDE.md` |
| `docs/` | 公开文档（ARCHITECTURE、API 等） | — |
| `CONTRACT.md` | 客户端/服务端协议 SSoT | — |
| `CHANGELOG.md` | 版本变更记录 | — |

内部 PRD / spec / roadmap 在 Codeup 仓库，不在本仓库。

---

## Git 规则

### main 分支保护

所有变更**必须通过 PR 合并**，禁止直接 push 到 main。

必须通过的 CI checks（9 个）：
`Lint·Rust` / `Lint·Go` / `Build&test·Go` / `Build&test·Java` / `Build&test·Rust` / `Coverage·Go` / `Coverage·Java` / `Coverage·Rust` / `E2E`

### push 必须绕过代理

本机 HTTPS 代理会导致 `LibreSSL SSL_ERROR_SYSCALL`，所有 push/fetch 必须：

```bash
git -c http.proxy="" -c https.proxy="" push origin <branch>
git -c http.proxy="" -c https.proxy="" fetch origin
```

### 分支生命周期

PR 合并或关闭后，对应分支**立即删除**（本地 + 远端）。

```bash
# 删除所有本地已合并分支（保留 main）
git branch | grep -v "^\* main" | xargs git branch -D

# 删除所有远端分支（保留 main）
git branch -r | grep -v "HEAD\|origin/main" | sed 's|origin/||' | \
  xargs -I{} git -c http.proxy="" -c https.proxy="" push origin --delete {}

# 清理本地过期远端引用
git -c http.proxy="" -c https.proxy="" fetch --prune origin
```

---

## 安全红线

- `RELEASE_SIGNING_KEY`（ed25519 私钥）：**绝不打印到 stdout，绝不 commit**
- ed25519 公钥已硬编码在 `client/src/update.rs`，不得修改
- 逆向来源、内部安全审计 ID、"limix"：**绝不出现在任何提交或文件中**
- `hmac_secret` / `AITRACK_SECRET_KEY` / `AITRACK_ADMIN_KEY`：不得出现在代码或日志中

---

## 版本规则

- 格式：`v1.0.0`，patch 自增（`v1.0.0 → v1.0.1`）
- tag 由 release CI 自动打，不手动创建
- Commit 前缀：`fix:` `feat:` `docs:` `ci:` `refactor:` `test:`
