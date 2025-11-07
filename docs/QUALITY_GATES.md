# 质量门禁配置说明

本文档说明项目的完整质量保障体系，包括本地 hooks、远程 CI 和配置文件。

## 质量保障三层体系

### 第一层：pre-commit hook（快速反馈，<10秒）

**目标**：在提交前进行快速检查，不阻塞开发流程

**检查项**：

1. 更新文档目录树
2. lint-staged（只检查暂存的文件）
3. prettier 格式化（自动修复）

**配置文件**：`.husky/pre-commit`

**为什么不在 commit 时运行测试？**

- commit 应该快速（<10秒），频繁的测试会影响开发体验
- 测试放在 push 阶段，作为本地的最后防线

---

### 第二层：pre-push hook（完整验证，可以较慢）

**目标**：在推送前进行完整验证，确保本地代码质量

**检查项**：

1. 验证文档目录树
2. prettier 格式检查
3. TypeScript 类型检查
4. ESLint 完整检查
5. **完整测试套件**
6. 构建验证

**配置文件**：`.husky/pre-push`

**为什么在 push 时运行测试？**

- push 是本地的最后一道防线
- 确保推送到远程的代码已经过完整验证
- 避免远程 CI 失败，节省团队时间

---

### 第三层：GitHub Actions CI（最完整验证）

**目标**：在远程进行最完整的验证，包括多平台测试

**检查项**：

1. Rust core 格式检查
2. Rust core 测试
3. Native addon 构建（napi）
4. Native addon 测试
5. WASM 模块构建
6. TypeScript 类型检查
7. ESLint 检查
8. Prettier 格式检查
9. **完整测试 + 覆盖率检查（≥75%）**
10. 构建验证
11. 临时文件清理检查
12. **多平台测试**（Linux/macOS/Windows）

**配置文件**：`.github/workflows/ci.yml`

**为什么需要远程 CI？**

- 多平台验证（本地只能测试一个平台）
- 覆盖率门禁（≥75%）
- Rust/WASM 构建验证
- 作为最后的安全网

---

## PR 质量门禁

### PR 模板

**配置文件**：`.github/pull_request_template.md`

**必填项**：

- 改动说明
- **关联 Issue**（必须包含 `Closes #<number>`）
- 测试清单
- 检查清单

### PR 自动检查

**配置文件**：`.github/workflows/pr-check.yml`

**检查项**：

1. PR 描述必须包含 `Closes #<number>`
2. Issue 必须存在
3. Issue 必须为 open 状态

**为什么需要这个检查？**

- 确保所有改动都有对应的 Issue
- 自动关闭 Issue，保持 Milestone 进度同步
- 便于追踪和回溯

---

## 配置文件清单

### Git Hooks

- `.husky/pre-commit` - commit 前快速检查
- `.husky/pre-push` - push 前完整验证

### GitHub Actions

- `.github/workflows/ci.yml` - 主 CI 流水线
- `.github/workflows/pr-check.yml` - PR 检查

### 代码质量工具

- `eslint.config.js` - ESLint 配置
- `.prettierrc` - Prettier 配置
- `tsconfig.json` - TypeScript 配置
- `vitest.config.ts` - 测试配置
- `.lintstaged.cjs` - lint-staged 配置

### 文档

- `.github/pull_request_template.md` - PR 模板
- `docs/architecture/index.md` - ADR 索引
- `.agents/rules/base.md` - 开发流程规范（v5.5）

---

## 常见问题

### Q: 为什么 commit 和 push 都有检查？

A: 这是分层验证策略：

- commit 时做快速检查（<10秒），不影响开发体验
- push 时做完整验证（包括测试），确保本地质量
- 远程 CI 做最完整验证（包括多平台），作为最后防线

### Q: 如果 commit 很慢怎么办？

A: commit 应该很快（<10秒）。如果慢，检查：

1. 是否有大量文件需要格式化？
2. lint-staged 配置是否正确？
3. 是否误触发了完整测试？

### Q: 如果 push 很慢怎么办？

A: push 时运行完整测试，可能需要 1-3 分钟。这是正常的。

- 如果超过 5 分钟，检查测试是否有性能问题
- 可以在 push 前先运行 `pnpm test` 确保测试通过

### Q: 可以跳过 hooks 吗？

A: **不可以**。使用 `--no-verify` 跳过 hooks 是违反项目规范的。

- 唯一例外：CI 配置本身有问题时，需要在 commit message 中说明原因

### Q: 远程 CI 失败了怎么办？

A: 如果本地 hooks 都通过了，远程 CI 不应该失败。如果失败：

1. 检查是否是多平台问题（如 Windows 路径）
2. 检查是否是 Rust/WASM 构建问题
3. 检查是否是覆盖率不足（<75%）

---

## 更新记录

- 2025-11-07: 初始版本，基于 base.md v5.5 标准
