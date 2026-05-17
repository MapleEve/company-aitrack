[English](#english)

## 中文

## 变更说明

<!-- 简要说明这个 PR 为 aitrack 带来的用户侧或维护侧变化 -->

## 变更类型

<!-- 用 x 勾选最贴切的项 -->

- [ ] 缺陷修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新
- [ ] 代码重构
- [ ] 性能优化
- [ ] 依赖更新

## 关联问题

<!-- 用 #123 关联 issue；没有则留空 -->

Fixes #
Relates to #

## 具体改动

<!-- 列出主要改动，不要贴大段实现细节 -->

-
-
-

## 截图（如适用）

<!-- UI 或接口变化建议附截图 -->

## 测试

<!-- 写清你实际跑过的命令和关键结果 -->

### 测试环境

- OS:
- Rust version (if client):
- Java version (if server-java):
- Go version (if server-go):
- Docker version:

### 测试步骤

1.
2.
3.

## 自检清单

<!-- 用 x 勾选已完成项 -->

- [ ] 代码符合项目风格
- [ ] 已完成自审
- [ ] 难理解代码已有必要注释
- [ ] 已同步更新文档
- [ ] 没有引入新的警告或错误
- [ ] 已补充有效测试
- [ ] Docker 镜像构建通过（`docker build -f docker/Dockerfile.<component> .`）
- [ ] 如涉及 client，`cargo test` 本地通过
- [ ] 如涉及 server-java，`mvn verify` 本地通过
- [ ] 如涉及 server-go，`go test ./...` 本地通过
- [ ] 如涉及 e2e，`bash e2e/run.sh both` 本地通过
- [ ] 日志、截图、fixture 均已脱敏，不包含凭据、token、hmac_secret、admin key、私人 diff 内容或内部路径

## API / 协议变更

<!-- 如果涉及 client-server 接口（/edits、/heartbeat、/stats、/admin/tokens）或签名协议，请补全下面内容 -->

- [ ] CONTRACT.md 已更新
- [ ] 已本地验证向后兼容性或已说明迁移路径
- [ ] e2e 测试已覆盖变更后的行为

## 破坏性变更

<!-- 若本 PR 引入破坏性变更，请描述影响范围和迁移方式 -->

## 补充说明

<!-- 任何 reviewer 应知道的额外信息 -->

## 给 Reviewers

<!-- 指出最值得 review 的点，例如签名逻辑、anti-cheat 判定、覆盖率门禁 -->

-
-

---

## English

Use the same sections above in English when needed for external contributors:

- Description
- Type of Change
- Related Issues
- Changes Made
- Screenshots
- Testing
- Checklist
- API / Protocol Changes
- Breaking Changes
- Additional Notes
- For Reviewers
