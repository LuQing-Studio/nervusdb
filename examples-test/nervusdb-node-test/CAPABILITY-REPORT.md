# NervusDB Node Binding — 能力边界测试报告

> 测试日期: 2026-02-16
> 测试环境: macOS Darwin 24.6.0 / Node.js v22.17.0
> NervusDB 版本: nervusdb-node (N-API, release build)
> 测试用例: 105 项 (99 passed / 5 failed / 1 skipped)

---

## 一、API 表面

```typescript
// nervusdb-node/index.d.ts
class Db {
  static open(path: string): Db
  query(cypher: string): QueryRow[]      // 只读查询
  executeWrite(cypher: string): number   // 写操作 (返回受影响行数)
  beginWrite(): WriteTxn                 // 开启写事务
  close(): void
}

class WriteTxn {
  query(cypher: string): void   // 暂存写语句
  commit(): number              // 提交 (返回受影响行数)
  rollback(): void              // 回滚 (清空暂存)
}
```

返回值类型: `QueryRow = Record<string, QueryValue>`

| QueryValue 类型 | JSON 表示 |
|----------------|-----------|
| null | `null` |
| boolean | `true` / `false` |
| number (int) | `42` |
| number (float) | `3.14` |
| string | `"hello"` |
| list | `[1, 2, 3]` |
| map | `{a: 1, b: "two"}` |
| node | `{type: "node", id, labels, properties}` |
| relationship | `{type: "relationship", src, dst, rel_type, properties}` |
| path | `{type: "path", nodes, relationships}` |

---

## 二、完全可用的功能 ✅

### 2.1 CRUD 操作
- [x] `CREATE` 单节点 (含标签 + 属性)
- [x] `CREATE` 关系 (含类型 + 属性)
- [x] `CREATE` 链式路径 `(a)-[:R]->(b)-[:R]->(c)`
- [x] `MATCH` + `RETURN` 节点/关系/属性
- [x] `SET` 新增属性 / 覆盖属性
- [x] `REMOVE` 属性
- [x] `DELETE` 关系
- [x] `DETACH DELETE` 节点 (含关联关系)

### 2.2 数据类型
- [x] null / boolean / integer / float / string
- [x] 负整数
- [x] 空字符串
- [x] 大字符串 (10,000 字符验证通过)
- [x] list 字面量 (`[1, 2, 3]`)
- [x] map 字面量 (`{a: 1, b: 'two'}`)
- [x] list 属性存储在节点上
- [x] 节点支持 50+ 属性

### 2.3 WHERE 过滤
- [x] 等值: `=`
- [x] 比较: `>`, `<`, `>=`, `<=`, `<>`
- [x] 逻辑: `AND`, `OR`, `NOT`
- [x] `IN [...]`
- [x] `STARTS WITH` / `CONTAINS` / `ENDS WITH`
- [x] `IS NULL` / `IS NOT NULL`

### 2.4 查询子句
- [x] `ORDER BY` (ASC / DESC)
- [x] `SKIP` / `LIMIT`
- [x] `WITH` (管道 + 过滤)
- [x] `UNWIND`
- [x] `UNION` / `UNION ALL`
- [x] `OPTIONAL MATCH`
- [x] `RETURN DISTINCT`
- [x] `RETURN *`
- [x] `RETURN` 表达式别名 (`AS`)
- [x] `RETURN` 无 MATCH 的字面量

### 2.5 聚合函数
- [x] `count()` / `count(DISTINCT ...)`
- [x] `sum()` / `avg()` / `min()` / `max()`
- [x] `collect()`
- [x] 隐式 GROUP BY

### 2.6 MERGE (节点)
- [x] `MERGE` 不存在时创建
- [x] `MERGE` 已存在时匹配 (不重复创建)
- [x] `MERGE ... ON CREATE SET`
- [x] `MERGE ... ON MATCH SET`

### 2.7 表达式
- [x] `CASE ... WHEN ... THEN ... ELSE ... END` (简单形式)
- [x] `CASE WHEN cond THEN ... END` (通用形式)
- [x] 算术: `+`, `-`, `*`, `/`, `%`
- [x] `abs()`, `sign()`, `toInteger()`, `toFloat()`

### 2.8 字符串函数
- [x] `toString()`
- [x] `toUpper()` / `toLower()`
- [x] `trim()` / `lTrim()` / `rTrim()`
- [x] `substring(str, start, len)`
- [x] `size()` (字符串长度)
- [x] `replace()`

### 2.9 图模式
- [x] 出方向 `->` / 入方向 `<-` / 无向 `-`
- [x] 关系属性读写
- [x] 自环关系 `(n)-[:SELF]->(n)`
- [x] 三角形模式匹配
- [x] 多跳 + WHERE 过滤
- [x] 多 MATCH 子句 (笛卡尔积)

### 2.10 变长路径
- [x] 固定长度 `*2`
- [x] 范围 `*1..3`
- [x] 上界 `*..2`

### 2.11 高级特性
- [x] `EXISTS { pattern }` 子查询
- [x] `FOREACH (i IN list | CREATE ...)`
- [x] `UNWIND` + `CREATE` 批量写入

### 2.12 其他
- [x] 持久化: close + reopen 数据不丢失
- [x] 空结果集返回 `[]`
- [x] 错误处理: 结构化 JSON payload (`{code, category, message}`)
- [x] close 后操作抛出 `"database is closed"` 错误
- [x] double close 安全 (不抛异常)
- [x] `query()` 拒绝写操作 (读写分离)

---

## 三、已知限制 ❌

### 3.1 [BUG] 单标签匹配多标签节点失败

**现象**: 创建 `(n:Person:Employee:Manager)` 后，`MATCH (n:Manager)` 返回空。

**预期**: 应该能通过任意单个标签子集匹配到节点。

**影响**: 中等。多标签节点的查询灵活性受限。

**绕过方案**: 查询时使用完整标签组合，或设计时只用单标签 + `type` 属性区分。

```cypher
-- 失败
MATCH (n:Manager) RETURN n  -- 返回空

-- 成功
MATCH (n:Person:Employee:Manager) RETURN n  -- 正常返回
MATCH (n:Person:Employee) RETURN n  -- 需要测试是否可行
```

---

### 3.2 [BUG] MERGE 关系可能不正确

**现象**: `MERGE (a)-[:LINK]->(b)` 执行后，`count(r)` 返回 0。

**预期**: MERGE 应创建关系并可查询到。

**影响**: 高。MERGE 关系是常用操作。

**绕过方案**: 用 `MATCH` + `CREATE` 手动实现幂等关系创建。

```typescript
// 绕过: 先查后建
const existing = db.query("MATCH (a:MA)-[r:LINK]->(b:MB) RETURN count(r) AS c");
if ((existing[0].c as number) === 0) {
  db.executeWrite("MATCH (a:MA), (b:MB) CREATE (a)-[:LINK]->(b)");
}
```

---

### 3.3 [MISSING] `left()` / `right()` 字符串函数未实现

**现象**: `RETURN left('hello', 3)` 报 `UnknownFunction`。

**影响**: 低。

**绕过方案**: 用 `substring()` 替代。

```cypher
-- left('hello', 3) 替代:
RETURN substring('hello', 0, 3) AS l

-- right('hello', 3) 替代:
RETURN substring('hello', size('hello') - 3) AS r
```

---

### 3.4 [BUG] WriteTxn commit 返回 0 / 数据不可见

**现象**: `beginWrite()` → `txn.query("CREATE ...")` → `txn.commit()` 返回 0，且数据查不到。

**根因**: WriteTxn 的 `commit()` 内部重新 `RustDb::open(path)` 打开新连接，导致与主 `Db` 实例的快照不一致。

```rust
// nervusdb-node/src/lib.rs:229
pub fn commit(&mut self) -> Result<u32> {
    let db = RustDb::open(&self.db_path).map_err(napi_err_v2)?;  // ← 新连接
    // ...
}
```

**影响**: 高。WriteTxn 基本不可用。

**绕过方案**: 不使用 `beginWrite()`，改用 `executeWrite()` 逐条写入。

```typescript
// 不要用:
const txn = db.beginWrite();
txn.query("CREATE (:A)");
txn.query("CREATE (:B)");
txn.commit();  // ← 返回 0，数据丢失

// 改用:
db.executeWrite("CREATE (:A)");
db.executeWrite("CREATE (:B)");
```

---

### 3.5 [MISSING] `shortestPath()` 不支持

**现象**: `shortestPath((a)-[:R*]->(b))` 解析失败。

**影响**: 低。图遍历场景可用变长路径近似。

**绕过方案**: 用变长路径 + `LIMIT 1`。

```cypher
MATCH (a:V {name: 'A'})-[:NEXT*1..10]->(d:V {name: 'D'})
RETURN length(collect(d)) AS hops
LIMIT 1
```

---

### 3.6 [LIMITATION] 单语句多节点 CREATE

**现象**: `CREATE (:A {v: 1}), (:B {v: 2})` 报 `duplicate external id in same tx`。

**影响**: 中等。批量创建需要逐条执行或用 UNWIND。

**绕过方案**:

```typescript
// 方案 A: 逐条
db.executeWrite("CREATE (:A {v: 1})");
db.executeWrite("CREATE (:B {v: 2})");

// 方案 B: UNWIND
db.executeWrite("UNWIND [{l: 'A', v: 1}, {l: 'B', v: 2}] AS item CREATE (:Node {label: item.l, v: item.v})");
```

> 注: 链式 CREATE 如 `CREATE (a:X)-[:R]->(b:Y)` 是正常的，问题仅出现在逗号分隔的多独立节点。

---

## 四、性能基线

| 操作 | 耗时 | 吞吐量 |
|------|------|--------|
| 单条 `executeWrite` (1000 次循环) | 14,408ms | ~69 ops/s |
| `UNWIND` 批量创建 100 节点 | 1,079ms | ~93 ops/s |
| 查询 1000 节点 (ORDER BY + LIMIT) | 9ms | ~111,000 rows/s |

**结论**: 读性能优秀，写性能受限于每次 commit 的开销。对于记忆系统场景（低频写入、高频读取）完全够用。

---

## 五、对 nervusdb-men 项目的设计建议

1. **写入策略**: 全部使用 `executeWrite()`，不依赖 `WriteTxn`
2. **节点设计**: 避免多标签，用单标签 + `type` 属性区分实体类型
3. **关系创建**: 用 `MATCH` + `CREATE` 替代 `MERGE` 关系
4. **批量写入**: 优先用 `UNWIND` + `CREATE`，避免逗号分隔多节点
5. **字符串处理**: 用 `substring()` 替代 `left()` / `right()`
6. **路径查询**: 用变长路径 `*1..N` 替代 `shortestPath()`

---

## 六、待修复项 (反馈给 nervusdb 核心)

| 优先级 | 问题 | 文件 | 建议 |
|--------|------|------|------|
| P0 | WriteTxn commit 重新 open DB | `nervusdb-node/src/lib.rs:229` | 复用主 Db 实例的连接 |
| P0 | 单语句多节点 CREATE 报 duplicate id | nervusdb-storage | 修复 WAL external id 分配 |
| P1 | 单标签匹配多标签节点失败 | nervusdb-query | 标签匹配应支持子集 |
| P1 | MERGE 关系不工作 | nervusdb-query | 修复 MERGE 关系路径 |
| P2 | 缺少 left() / right() | nervusdb-query/evaluator | 添加函数实现 |
| P2 | 缺少 shortestPath() | nervusdb-query | 添加最短路径算法 |
