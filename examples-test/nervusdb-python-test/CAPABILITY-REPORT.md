# NervusDB Python Binding — 能力边界测试报告

## 概要

| 指标 | 数值 |
|------|------|
| 总测试数 | 135 |
| 通过 | 131 |
| 失败 | 3 |
| 跳过 | 1 |
| 通过率 | 97.0% |
| 测试分类 | 27 类（20 类镜像 Node.js + 7 类 Python 独有） |

## 失败列表

| # | 测试名 | 错误信息 | 同 Node.js? |
|---|--------|---------|------------|
| 1 | MATCH by single label subset | `should match by Manager label` — `MATCH (n:Manager)` 无法匹配 `Person:Employee:Manager` 节点 | 是 |
| 2 | MERGE relationship | `0 != 1` — MERGE 关系后 count 为 0，说明 MERGE 关系根本不工作 | 是 |
| 3 | left / right | `syntax error: UnknownFunction` — `left()` 和 `right()` 函数未实现 | 是 |

## 跳过列表

| # | 测试名 | 原因 |
|---|--------|------|
| 1 | shortestPath | 未实现 |

## 与 Node.js 对比

### 共同 Bug（Python 和 Node.js 都失败）

1. **单标签匹配多标签节点** — `MATCH (n:Manager)` 无法匹配带有 `Person:Employee:Manager` 三个标签的节点。多标签创建和双标签匹配正常，但单标签子集匹配失败。
2. **MERGE 关系不工作** — `MATCH (a:MA), (b:MB) MERGE (a)-[:LINK]->(b)` 执行后关系 count 为 0。MERGE 节点正常，但 MERGE 关系完全不工作。
3. **left() / right() 未实现** — 这两个 Cypher 标准字符串函数返回 `UnknownFunction` 错误。

### Python 修复的 Bug（Node.js 失败但 Python 通过）

1. **WriteTxn commit** — Node.js 中 `commit()` 返回 0 且会重开 DB（bug），Python 中 `commit()` 返回 None 且正常工作。
2. **多节点 CREATE** — Node.js 中 `CREATE (:A), (:B)` 可能报 "duplicate external id"，Python 中正常通过。

### Python 独有功能测试结果

| 分类 | 测试数 | 通过 | 状态 |
|------|--------|------|------|
| query_stream() | 4 | 4 | 全部通过 |
| 参数化查询 | 5 | 5 | 全部通过 |
| 向量操作 | 4 | 4 | 全部通过 |
| 类型化对象 | 5 | 5 | 全部通过 |
| 异常层级 | 5 | 5 | 全部通过 |
| Db.path + open() | 3 | 3 | 全部通过 |
| Python 边界情况 | 5 | 5 | 全部通过 |

## 分类详细结果

### 1. 基础 CRUD — 9/9 通过
CREATE / MATCH / SET / REMOVE / DELETE 全部正常。多节点 CREATE 也通过。

### 1b. RETURN 投影 — 4/4 通过
标量表达式、属性别名、DISTINCT、RETURN * 全部正常。

### 2. 多标签节点 — 1/2 通过
多标签创建和双标签匹配正常，单标签子集匹配失败。

### 3. 数据类型 — 9/9 通过
null / bool / int / float / string / list / map 全部正常。

### 4. WHERE 过滤 — 11/11 通过
所有比较运算符、逻辑运算符、字符串谓词、NULL 检查全部正常。

### 5. 查询子句 — 10/10 通过
ORDER BY / LIMIT / SKIP / WITH / UNWIND / UNION / OPTIONAL MATCH 全部正常。

### 6. 聚合函数 — 7/7 通过
count / sum / avg / min / max / collect / count(DISTINCT) / GROUP BY 全部正常。

### 7. MERGE — 4/5 通过
MERGE 节点（创建/匹配/ON CREATE SET/ON MATCH SET）正常，MERGE 关系失败。

### 8. CASE 表达式 — 2/2 通过

### 9. 字符串函数 — 6/7 通过
toString / toUpper / toLower / trim / substring / size / replace 正常，left/right 未实现。

### 10. 数学运算 — 4/4 通过

### 11. 变长路径 — 3/4 通过（1 跳过）
固定长度和变长路径正常，shortestPath 未实现（跳过）。

### 12. EXISTS 子查询 — 1/1 通过

### 13. FOREACH — 1/1 通过

### 14. 事务 (WriteTxn) — 4/4 通过
commit / rollback / 语法错误 / 独立事务全部正常。Python 事务实现比 Node.js 更健壮。

### 15. 错误处理 — 6/6 通过
类型化异常、关闭后操作、双重关闭全部正常。

### 16. 关系方向 — 4/4 通过

### 17. 复杂图模式 — 3/3 通过

### 18. 批量写入性能 — 3/3 通过
- 1000 节点逐条创建：~16s（61 ops/s，debug 模式）
- 查询 1000 节点：49ms
- UNWIND 100 节点批量创建：~1.2s

### 19. 持久化 — 1/1 通过

### 20. 边界情况 — 6/6 通过

### 21-27. Python 独有 — 31/31 通过

## 关键发现

1. **Python 绑定质量优于 Node.js** — 131/135 通过（97%）vs Node.js 99/105（94.3%）
2. **WriteTxn 在 Python 中正常工作** — 这是 Python 绑定的重要优势
3. **3 个共同 Bug 来自 Rust 核心引擎** — 不是绑定层的问题
4. **Python 独有功能全部正常** — query_stream、参数化查询、向量搜索、类型化异常等
5. **close() 行为更安全** — 有活跃事务时正确抛出 StorageError
