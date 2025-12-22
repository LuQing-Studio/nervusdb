# T6: 冻结并对齐 `nervusdb.h`（最小稳定 C 契约）

## 1. Context

- 现在的 `nervusdb-core/include/nervusdb.h` 只暴露了少数函数，但 `nervusdb-core/src/ffi.rs` 实际已经实现了更多能力（路径算法等）。
- “接口一旦公开就是契约”。1.0 之前不收口，1.0 之后就会被自己打脸。

## 2. Goals

- 定义并冻结一套极简、可长期维持的 C API：
  - `nervusdb_open / nervusdb_close`
  - 字典：`intern / resolve`（如果需要）
  - 数据：`add_triple / query_triples`
  - 查询：`exec_cypher`（最小可用子集）
  - 错误处理与内存释放规则（谁分配谁释放，写死）
- 头文件与 Rust 实现 100% 对齐（禁止“实现有但头文件没写”的状态）。

## 3. Non-Goals

- 不把所有内部特性都塞进 C API（插件/算法/高级 API 都不应成为 1.0 的负担）。

## 4. Solution

1. 先对齐现有暴露面：
   - 清点 `ffi.rs` 实际导出的 symbol，与 `nervusdb.h` 做一致化。
2. 增量引入 `exec_cypher`：
   - 输出 **JSON 字符串**（最蠢最清楚），并提供 `nervusdb_free_string()` 统一释放。
3. 加版本机制：
   - 例如 `nervusdb_version()` 或宏常量，避免“盲猜 ABI”。
4. 砍掉未公开的导出：
   - `ffi.rs` 里存在但 `nervusdb.h` 未声明的算法导出（BFS/Dijkstra/PageRank）不进入 1.0 契约：要么删掉，要么 feature gate（默认关）。

## 5. Testing Strategy

- C 示例程序可编译可运行（最小 smoke test）。
- Rust 侧对 FFI 的基本 ABI 行为做单测（错误释放、空指针保护）。

## 6. Risks

- ABI 一旦冻结就不能乱改：需要明确哪些能力“永远不进核心”。
