# NervusDB Node.js 绑定 — 缺失能力报告

> 基于 Python (PyO3) 绑定与 Node.js (napi) 绑定的源码对比分析
> 生成日期: 2026-02-16

## 概要

| 指标 | Node.js | Python | 差距 |
|------|---------|--------|------|
| 源码规模 | 1 文件 / 313 行 | 5 文件 / ~500 行 | Node 为最小原型 |
| 暴露 API 数 | 6 | 14 | Node 缺少 8 个 API |
| 测试通过率 | 94.3% (99/105) | 97.0% (131/135) | — |
| 参数化查询 | 不支持 | 支持 | 缺失 |
| 向量操作 | 不支持 | 支持 | 缺失 |
| 流式查询 | 不支持 | 支持 | 缺失 |
| 类型化异常 | JSON payload | 原生异常层级 | 设计差异 |
| WriteTxn 架构 | 假事务（缓冲+重开） | 真事务（持有 Rust 对象） | 架构缺陷 |

## 缺失能力详细分析

### 1. 参数化查询 (params)

**严重程度: 高** — 安全性和易用性的基础功能

| | Node.js | Python |
|--|---------|--------|
| API | `db.query(cypher)` | `db.query(cypher, params={"name": "Alice"})` |
| 实现 | 硬编码 `&Params::new()` | 完整的 `py_to_value()` 类型转换 |

**根因:** Node.js `run_query()` (lib.rs:125-143) 和 `execute_write()` (lib.rs:172-185) 都没有接受 params 参数，直接使用空 `Params::new()`。Rust 核心的 `nervusdb_query::Params` 完全支持参数化，但 Node.js 绑定缺少 JS 对象 → `Value` 的类型转换层。

**影响:**
- 无法防止 Cypher 注入（所有查询必须拼接字符串）
- 无法传递复杂类型（list/map/null）作为参数
- 与 Neo4j 驱动等标准 Cypher 客户端的 API 不一致

**修复方案:** 在 `query()` 和 `execute_write()` 中添加可选的 `params: Option<HashMap<String, JsUnknown>>` 参数，实现 `js_to_value()` 转换函数（参考 Python 的 `py_to_value()`，约 50 行）。

---

### 2. 向量操作 (search_vector / set_vector)

**严重程度: 高** — 核心差异化功能完全缺失

| | Node.js | Python |
|--|---------|--------|
| search_vector | 不存在 | `db.search_vector(query_vec, k)` → `[(node_id, distance)]` |
| set_vector | 不存在 | `txn.set_vector(node_id, vector)` |

**根因:** Node.js 绑定完全没有暴露 Rust 核心的 `db.search_vector()` 和 `txn.set_vector()` 方法。

**影响:**
- Node.js 用户无法使用 NervusDB 的向量搜索能力
- 无法构建 RAG、语义搜索等 AI 应用场景

**修复方案:**
- 在 `Db` 上添加 `search_vector(query: Vec<f64>, k: u32)` 方法
- 在 `WriteTxn` 上添加 `set_vector(node_id: u32, vector: Vec<f64>)` 方法
- 需要先修复 WriteTxn 架构（见第 5 项）才能正确实现 `set_vector`

---

### 3. 流式查询 (query_stream)

**严重程度: 中** — 大结果集场景的性能优化

| | Node.js | Python |
|--|---------|--------|
| API | 不存在 | `db.query_stream(cypher, params)` → 迭代器 |
| 特性 | — | `.len` 属性、`__iter__`/`__next__` 协议 |

**根因:** Node.js 绑定没有实现。Python 的 `QueryStream` 当前也是先 materialize 再迭代（非真正流式），但 API 已经稳定，未来可以无缝升级为真正的流式。

**影响:**
- 大结果集必须一次性加载到内存
- 无法实现惰性处理模式

**修复方案:** 实现 napi 的 `Iterator` 或返回 `Generator` 对象，包装 materialized rows。约 40 行代码。

---

### 4. 类型化异常

**严重程度: 中** — 错误处理的开发体验

| | Node.js | Python |
|--|---------|--------|
| 异常类型 | 通用 `Error`，message 是 JSON 字符串 | `NervusError` > `SyntaxError` / `ExecutionError` / `StorageError` / `CompatibilityError` |
| 捕获方式 | `try { } catch(e) { JSON.parse(e.message) }` | `except nervusdb.SyntaxError as e:` |

**根因:** Node.js 选择了 JSON payload 方案（`error_payload(code, category, message)`），所有错误都是通用 `napi::Error`。napi-rs 支持自定义 Error 类，但需要更多代码。

**影响:**
- 用户必须解析 JSON 字符串才能区分错误类型
- 无法用 `instanceof` 进行类型检查
- 错误处理代码冗长且易出错

**修复方案:** 使用 napi-rs 的 `#[napi]` 宏定义自定义 Error 类，或至少在 TypeScript 层面提供 `NervusError` 包装类和 `isSyntaxError()` 等辅助函数。

---

### 5. WriteTxn 架构缺陷

**严重程度: 极高** — 核心事务机制的设计缺陷

| | Node.js | Python |
|--|---------|--------|
| 内部结构 | `staged_queries: Vec<String>` | `inner: Option<RustWriteTxn<'static>>` |
| query() | push 字符串到 Vec | 直接在 Rust 事务上执行 |
| commit() | **重新 `RustDb::open(path)`** 打开新 DB 实例 | 直接 `txn.commit()` |
| rollback() | 清空 Vec（commit 仍可调用） | 结束事务（再调 commit 抛异常） |
| set_vector | 无法实现 | 直接调用 `txn.set_vector()` |

**根因:** Node.js 的 `WriteTxn` 是"假事务"。`commit()` 的实现（lib.rs:228-243）：

```rust
pub fn commit(&mut self) -> Result<u32> {
    let db = RustDb::open(&self.db_path).map_err(napi_err_v2)?;  // ← 重新打开 DB！
    let snapshot = db.snapshot();
    let mut txn = db.begin_write();
    // ... 在新实例上执行所有缓存的查询 ...
    txn.commit().map_err(napi_err)?;
    Ok(total)
}
```

这意味着：
1. commit 操作的是一个**全新的 DB 实例**，不是用户持有的那个
2. 依赖 NervusDB 存储引擎的多实例文件共享来同步数据
3. 无法实现 `set_vector`（需要真正的事务对象）
4. rollback 后仍可 commit（只是执行空查询列表）
5. 没有真正的事务隔离

Python 绑定通过 `unsafe transmute` 延长 Rust 事务的生命周期，持有真正的 `RustWriteTxn` 对象，实现了正确的事务语义。

**影响:**
- 事务不是真正的原子操作
- 无法实现 set_vector
- rollback 语义不正确
- 每次 commit 都有重新打开 DB 的开销

**修复方案:** 参考 Python 的 `txn.rs` 实现，让 `WriteTxn` 持有真正的 Rust 事务对象。需要处理 napi-rs 中的生命周期问题（可以用 `Arc` + `Mutex` 或类似 Python 的 `unsafe transmute` 方案）。这是最大的改动，约 80-100 行。

---

### 6. 其他缺失 API

| 缺失项 | 严重程度 | 说明 |
|--------|---------|------|
| `Db.path` 属性 | 低 | Python 有 `#[getter] fn path()`，Node.js 没有暴露 |
| `nervusdb.open()` 便捷函数 | 低 | Python 有顶层 `open()` 函数，Node.js 只有 `Db.open()` |
| `close()` 活跃事务检查 | 中 | Python 在有活跃事务时抛 `StorageError`，Node.js 直接关闭不检查 |
| 返回值类型化 | 中 | Python 返回 `Node`/`Relationship`/`Path` 原生类型，Node.js 返回 JSON 普通对象 |

## 修复优先级建议

| 优先级 | 项目 | 预估工作量 | 理由 |
|--------|------|-----------|------|
| P0 | WriteTxn 架构重写 | 80-100 行 | 核心事务机制有缺陷，阻塞 set_vector |
| P0 | 参数化查询 | 50-60 行 | 安全性基础，防止注入 |
| P1 | 向量操作 | 30-40 行 | 核心差异化功能，依赖 WriteTxn 修复 |
| P1 | 类型化异常 | 40-50 行 | 开发体验 |
| P2 | query_stream | 30-40 行 | 大结果集优化 |
| P2 | 返回值类型化 | 60-80 行 | 可选，JSON 方案也可用 |
| P3 | 其他小项 | 10-20 行 | path 属性、open() 函数、close() 检查 |

**总预估: ~300-400 行新增/修改代码**，可将 Node.js 绑定从"最小原型"升级为与 Python 绑定对等的"完整实现"。

## 参考文件

| 文件 | 路径 |
|------|------|
| Node.js 绑定源码 | `nervusdb-node/src/lib.rs` (313 行) |
| Node.js 类型定义 | `nervusdb-node/index.d.ts` (50 行) |
| Python Db 实现 | `nervusdb-pyo3/src/db.rs` (201 行) |
| Python WriteTxn | `nervusdb-pyo3/src/txn.rs` (107 行) |
| Python 类型系统 | `nervusdb-pyo3/src/types.rs` (258 行) |
| Python 异常定义 | `nervusdb-pyo3/src/lib.rs` (183 行) |
| Python QueryStream | `nervusdb-pyo3/src/stream.rs` (40 行) |
| Node.js 能力测试 | `nervusdb-node-test/src/test-capabilities.ts` |
| Python 能力测试 | `nervusdb-python-test/test_capabilities.py` |
