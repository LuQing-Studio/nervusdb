# Binding Parity Matrix (Rust Baseline)

- 更新时间: 2026-02-16
- 基线原则: 以 Rust 当前行为作为唯一基线；Node/Python 必须与 Rust 同态。
- 口径: 不用 skip 掩盖绑定差异；若 Rust 核心本身有缺口，三端统一断言该缺口。

## 范围
本矩阵覆盖以下三类能力：
1. `Db` 高层 API
2. `WriteTxn` 低层写事务 API
3. 模块级维护 API (`vacuum/backup/bulkload`)

## 能力矩阵
| 能力 | Rust | Node | Python | 说明 |
|---|---|---|---|---|
| `open(path)` | ✅ | ✅ | ✅ | 三端可用 |
| `open_paths/openPaths` | ✅ | ✅ | ✅ | Node: `openPaths`；Python: `open_paths` |
| `path` | ✅ | ✅ | ✅ | |
| `ndb_path/ndbPath` | ✅ | ✅ | ✅ | |
| `wal_path/walPath` | ✅ | ✅ | ✅ | |
| `query(cypher, params?)` | ✅ | ✅ | ✅ | 参数化三端对齐 |
| `execute_write/executeWrite(cypher, params?)` | ✅ | ✅ | ✅ | 写语句强制走写接口 |
| `begin_write/beginWrite` | ✅ | ✅ | ✅ | |
| `compact` | ✅ | ✅ | ✅ | |
| `checkpoint` | ✅ | ✅ | ✅ | |
| `create_index/createIndex` | ✅ | ✅ | ✅ | |
| `search_vector/searchVector` | ✅ | ✅ | ✅ | |
| `WriteTxn.query` | ✅ | ✅ | ✅ | |
| `WriteTxn.commit/rollback` | ✅ | ✅ | ✅ | |
| `WriteTxn.create_node/createNode` | ✅ | ✅ | ✅ | |
| `WriteTxn.get_or_create_label/getOrCreateLabel` | ✅ | ✅ | ✅ | |
| `WriteTxn.get_or_create_rel_type/getOrCreateRelType` | ✅ | ✅ | ✅ | |
| `WriteTxn.create_edge/createEdge` | ✅ | ✅ | ✅ | |
| `WriteTxn.tombstone_node/tombstoneNode` | ✅ | ✅ | ✅ | |
| `WriteTxn.tombstone_edge/tombstoneEdge` | ✅ | ✅ | ✅ | |
| `WriteTxn.set_node_property/setNodeProperty` | ✅ | ✅ | ✅ | |
| `WriteTxn.set_edge_property/setEdgeProperty` | ✅ | ✅ | ✅ | |
| `WriteTxn.remove_node_property/removeNodeProperty` | ✅ | ✅ | ✅ | |
| `WriteTxn.remove_edge_property/removeEdgeProperty` | ✅ | ✅ | ✅ | |
| `WriteTxn.set_vector/setVector` | ✅ | ✅ | ✅ | |
| `vacuum(path)` | ✅ | ✅ | ✅ | |
| `backup(path, backup_dir)` | ✅ | ✅ | ✅ | |
| `bulkload(path, nodes, edges)` | ✅ | ✅ | ✅ | Node 入参字段为 camelCase，Python 为 snake_case |

## 错误语义口径
- Rust: 原生 `Error` 分类。
- Node: 结构化 JSON payload (`code/category/message`)。
- Python: `NervusError` 体系（`SyntaxError/ExecutionError/StorageError/CompatibilityError`）。
- 统一规则: 绑定差异不放行；同一输入在三端应落到同类错误语义。

## 已知 Rust 核心同态缺口（非绑定差异）
以下为内核行为缺口，当前要求三端同态，不视为 binding gap：
1. 多标签子集匹配 `MATCH (n:Manager)` 在已知场景返回 0。
2. `left()` / `right()` 尚未实现（`UnknownFunction`）。
3. `shortestPath` 尚未完整支持。
4. `MERGE` 关系场景存在已知核心不稳定点（能力测试中已标注）。

## 阶段状态（P0）
- S1 口径统一与差异冻结: ✅
- S2 Node 行为一致性收敛: ✅（事务相关测试已收紧为硬断言）
- S3 API 面补齐: ✅（Node/Python 对齐 Rust 基线）
- S4 维护能力与高级能力对齐: ✅（`backup/vacuum/bulkload/search_vector/create_index` 已覆盖）
- S5 CI 阻断: ⏳（由 `scripts/binding_parity_gate.sh` 与 CI 接线保障）

## Gate 命令
- `bash examples-test/run_all.sh`
- `bash scripts/binding_parity_gate.sh`

通过标准:
1. Rust/Node/Python capability tests 全绿。
2. 输出 parity gate 报告到 `artifacts/tck/`。
3. 无 skip 放行绑定差异。
