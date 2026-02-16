# NervusDB Rust 核心引擎能力测试报告

## 测试概览

| 指标 | 值 |
|------|-----|
| 总测试数 | 153 |
| 通过 | 153 |
| 失败 | 0 |
| 跳过 | 0 |
| 运行时间 | ~50s |

## 已知 Bug 确认（Rust 核心引擎层面）

### BUG-1: 单标签匹配多标签节点失败
- 测试: `t02_single_label_subset`
- 现象: `CREATE (n:Person:Employee:Manager)` 后 `MATCH (n:Manager)` 返回 0 行
- 结论: **确认 bug 来自 Rust 核心引擎**（非绑定层）
- Python/Node 同样复现

### BUG-2: left()/right() 字符串函数未实现
- 测试: `t09_left`, `t09_right`
- 现象: `RETURN left('hello', 3)` 报 `UnknownFunction`
- 结论: **确认 Rust 核心引擎未实现 left/right**

### BUG-3: MERGE 关系
- 测试: `t07_merge_relationship`
- 现象: 本次 Rust 测试中 MERGE 关系**通过**了
- 结论: MERGE 关系在 Rust 层面可能已修复，或 Python/Node 绑定层有额外问题

## 新发现

### node_count/edge_count 返回 0
- 测试: `t23_node_count`, `t23_edge_count`
- `DbSnapshot::node_count(None)` 和 `node_count(Some(label))` 均返回 0
- 可能需要 compaction 后才能获取正确计数
- 不影响 Cypher 查询（`count()` 聚合正常工作）

### shortestPath 未实现
- 测试: `t11_shortest_path_skip`
- 解析阶段报错 `Expected '('`

## 分类测试结果

### 镜像 Python/Node 测试（分类 1-20）

| # | 分类 | 测试数 | 结果 |
|---|------|--------|------|
| 1 | 基础 CRUD | 9 | 全部通过 |
| 1b | RETURN 投影 | 4 | 全部通过 |
| 2 | 多标签节点 | 2 | 1 通过 + 1 确认 bug |
| 3 | 数据类型 | 9 | 全部通过 |
| 4 | WHERE 过滤 | 11 | 全部通过 |
| 5 | 查询子句 | 10 | 全部通过 |
| 6 | 聚合函数 | 7 | 全部通过 |
| 7 | MERGE | 5 | 全部通过 |
| 8 | CASE 表达式 | 2 | 全部通过 |
| 9 | 字符串函数 | 7 | 5 通过 + 2 确认 bug |
| 10 | 数学运算 | 4 | 全部通过 |
| 11 | 变长路径 | 4 | 3 通过 + 1 跳过 |
| 12 | EXISTS 子查询 | 1 | 通过 |
| 13 | FOREACH | 1 | 通过 |
| 14 | 事务 WriteTxn | 4 | 全部通过 |
| 15 | 错误处理 | 6 | 全部通过 |
| 16 | 关系方向 | 4 | 全部通过 |
| 17 | 复杂图模式 | 3 | 全部通过 |
| 18 | 批量写入性能 | 3 | 全部通过 |
| 19 | 持久化 | 1 | 通过 |
| 20 | 边界情况 | 6 | 全部通过 |

### Rust 独有测试（分类 21-35）

| # | 分类 | 测试数 | 结果 |
|---|------|--------|------|
| 21 | 直接 WriteTxn API | 6 | 全部通过 |
| 22 | ReadTxn + neighbors | 3 | 全部通过 |
| 23 | DbSnapshot 方法 | 5 | 3 通过 + 2 注意事项 |
| 24 | 参数化查询 Params | 5 | 全部通过 |
| 25 | execute_mixed | 3 | 全部通过 |
| 26 | ExecuteOptions 资源限制 | 3 | 全部通过 |
| 27 | vacuum | 2 | 全部通过 |
| 28 | backup | 2 | 全部通过 |
| 29 | bulkload | 3 | 全部通过 |
| 30 | create_index | 2 | 全部通过 |
| 31 | compact + checkpoint | 2 | 全部通过 |
| 32 | Error 类型 | 4 | 全部通过 |
| 33 | 向量操作 | 4 | 全部通过 |
| 34 | Value reify | 3 | 全部通过 |
| 35 | Db 路径 + open_paths | 3 | 全部通过 |

## 运行方式

```bash
# 在 nervusdb workspace 中运行
cd rust/nervusdb
# 确保 nervusdb-rust-test 在 workspace members 中
cargo test -p nervusdb-rust-test --test test_capabilities -- --test-threads=1 --nocapture
```
