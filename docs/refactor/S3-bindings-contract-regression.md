# S3：Bindings 契约回归（Python/Node）

更新时间：2026-02-11  
任务类型：Phase 2  
任务状态：Plan

## 1. 目标

- 验证并锁定 Python/Node 的错误分类与 payload 契约。
- 在重构过程中确保 bindings 行为不回退。
- 将契约回归固定为每个后续 PR 的执行项。

## 2. 边界

- 允许：补测试、补断言、补文档。
- 禁止：重定义错误模型、改外部类型语义。
- 禁止：引入不兼容 payload 字段变更。

## 3. 文件清单

### 3.1 必查/必改文件

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-pyo3/src/lib.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-pyo3/src/types.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-node/src/lib.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-node/index.d.ts`

### 3.2 测试与脚本

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-pyo3/tests/test_basic.py`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-pyo3/tests/test_vector.py`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/scripts/binding_smoke.sh`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/scripts/contract_smoke.sh`

## 4. 前置证据

- spec 中已定义跨语言错误模型与门禁：`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/docs/spec.md:24`；`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/docs/spec.md:42`
- tasks 中 M5-01 为 WIP：`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/docs/tasks.md:86`

## 5. 测试清单

1. Python 语义：
   - 异常类型映射：`Syntax/Execution/Storage/Compatibility`
   - 结构化 payload 完整性
2. Node 语义：
   - `code/category/message` 字段稳定
   - 异常路径与成功路径类型稳定
3. 门禁脚本：
   - `bash scripts/binding_smoke.sh`
   - `bash scripts/contract_smoke.sh`

## 6. 回滚步骤

1. 任一语言绑定契约破坏即回滚 PR。
2. 增加契约测试后再重提，不允许只改文档放行。

## 7. 完成定义（DoD）

- Python/Node 契约用例通过。
- 跨语言错误模型与 spec 保持一致。
- 所有相关 PR 的 bindings smoke/contract smoke 全绿。
