# S2：Storage 读路径边界治理（结构解耦）

更新时间：2026-02-11  
任务类型：Phase 2  
任务状态：Plan

## 1. 目标

- 在不改语义前提下，对读路径热点职责做结构解耦。
- 降低锁热点与快照桥接的耦合密度。
- 为后续性能阶段（Phase 2/Phase 3）提供清晰边界。

## 2. 边界

- 允许：内部模块切分、辅助结构抽取、读路径整理。
- 禁止：存储格式变更、事务语义变更、外部 API 变更。
- 禁止：写路径逻辑重写。

## 3. 文件清单

### 3.1 必改文件

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/engine.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/snapshot.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/api.rs`

### 3.2 可选新增

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/read_path/mod.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/read_path/scanner.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/read_path/materialize.rs`

## 4. 证据与前置

- engine 体量与复杂度高：`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/src/engine.rs:1`
- 任务路径已在架构路线中定义：`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-Architecture.md:1185`
- 需在 Phase 1a/1c 之后执行：`/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-Architecture.md:1179`

## 5. 测试清单

- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/tests/t51_snapshot_scan.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/tests/m1_graph.rs`
- `/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb/nervusdb-v2-storage/tests/t47_api_trait.rs`
- `bash scripts/workspace_quick_test.sh`

## 6. 回滚步骤

1. 读一致性/快照一致性失败，立即回滚 PR。
2. 回滚后增加最小复现测试，再拆分更细任务重做。

## 7. 完成定义（DoD）

- 读路径职责边界明确，模块耦合下降。
- 快照相关回归全绿。
- 不引入外部 API 与语义变化。
