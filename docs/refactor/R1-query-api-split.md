# R1：`query_api.rs` 结构拆分（行为等价）

更新时间：2026-02-11  
任务类型：Phase 1a  
任务状态：In Progress

## 1. 目标

- 将 `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api.rs` 从单体文件拆分为可维护模块。
- 保持对外入口函数、返回类型、错误分类完全不变。
- 通过全门禁确认行为等价。

## 2. 边界

- 允许：内部模块重组、私有函数迁移、模块 re-export。
- 禁止：对外函数签名变化、错误类别变化、语义修复混入。
- 禁止：顺手修改 `executor.rs`/`evaluator.rs` 业务逻辑。

## 3. 文件清单

### 3.1 必改文件

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/lib.rs`

### 3.2 新增文件（建议结构）

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/mod.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/entry.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/parse.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/validate.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/assemble.rs`

## 4. TDD 拆分步骤

1. 新增失败测试：覆盖 parse/validate/assemble 入口等价行为。
2. 最小实现：先搬迁私有 helper 到子模块，保持旧入口委托。
3. 重构：缩减 `query_api.rs`，只保留 re-export 与薄入口。
4. 回归：跑全门禁 + 受影响测试清单。

## 5. 测试清单

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t52_query_api.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t311_expressions.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t62_order_by_skip_test.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2/tests/t332_binding_validation.rs`

## 6. 风险与回滚

- 风险：入口组装顺序变化触发隐式语义变化。
- 检测：对照 R0 样本查询结果与错误类别。
- 回滚：单 PR 回滚，不跨任务修复。

## 7. 完成定义（DoD）

- `query_api.rs` 明显减重，职责拆分清晰。
- 全门禁通过，且受影响测试无新增失败。
- 对外入口与错误模型保持不变。

## 8. 当前进展（2026-02-11）

- 已完成切片 1：`internal path alias` helper 抽取到
  `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/internal_alias.rs`，
  并补 2 个单元测试。
- 已完成切片 2：`strip_explain_prefix` 抽取到
  `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-query/src/query_api/explain.rs`，
  并补 3 个单元测试。
