# NervusDB 性能分析报告

## 当前 Benchmark 结果（100万条数据）

| 数据库 | 插入/秒 | S?? 查询/秒 | ??O 查询/秒 |
|--------|---------|-------------|-------------|
| **SQLite** | 1,020,350 | 411,403 | 416,641 |
| **redb (raw)** | 194,816 | 553,886 | 563,001 |
| **NervusDB** | 465,685 | 727,418 | 952,317 |

## 性能差距

- NervusDB 插入约为 SQLite 的 **45.6%**（465K vs 1,020K，约慢 2.2 倍）
- NervusDB 在本基准的两类点查（S?? / ??O）上已不落后（主要得益于读事务/表句柄复用）

## 已完成的优化

### 1. 索引表精简：5 → 3（已实现）

- 现仅维护 `SPO / POS / OSP` 三张索引表（仍可覆盖常见 `S?? / ??O / P??` 以及 `S?O / ?PO` 等模式）
- 插入路径写放大显著下降：每条 triple 写入 3 次（不含 dictionary/props）

### 2. WriteTableHandles 缓存（已实现）

```rust
pub(crate) struct WriteTableHandles<'txn> {
    pub spo: Table<'txn, (u64, u64, u64), ()>,
    pub pos: Table<'txn, (u64, u64, u64), ()>,
    pub osp: Table<'txn, (u64, u64, u64), ()>,
    pub str_to_id: Table<'txn, &'static str, u64>,
    pub id_to_str: Table<'txn, u64, &'static str>,
}
```

- 写事务内复用表句柄，避免每次 `open_table()` 的重复开销

### 3. 字符串 LRU 缓存 + 单路径 intern（已实现）

- `WriteTableHandles` 内部维护字符串 **LRU** 缓存，减少热字符串反复 `str_to_id.get()` 的 B-Tree 查找
- `next_id` 在写事务开始时计算一次，写入过程中不再重复 `id_to_str.last()` 探测
- 统一 intern 路径，移除空库 `fast_intern` 的特殊分支（避免缓存容量边界导致的字典不一致风险）

### 4. 读事务/表句柄缓存（已实现）

- 复用 `ReadTransaction + ReadOnlyTable`（写入 commit 后通过 generation 失效）
- 避免查询路径每次 `begin_read/open_table` 的固定成本

## 仍存在的瓶颈

### 1. 插入仍慢于 SQLite

- 仍有 dictionary/序列化/多表写入等固定成本；要继续追近 SQLite，需要进一步压缩“每条事实”的写放大与分配次数。

### 2. Binding 侧的解码/resolve 开销

- Rust 核心的查询返回的是 `u64` ID 三元组；Node/Python 若逐条 `resolveStr()` 还原字符串，会引入额外往返与分配。

## 代码位置

主要文件：
- `nervusdb-core/src/storage/disk.rs` - 磁盘存储实现
- `nervusdb-core/src/storage/mod.rs` - Hexastore trait 定义
- `nervusdb-core/src/lib.rs` - Database API
- `nervusdb-core/examples/bench_compare.rs` - Benchmark 代码

关键结构/函数：
- `WriteTableHandles` - 写事务表句柄与字符串缓存
- `ReadHandles` - 读事务与表句柄缓存
- `DiskHexastore::read_handles()` - 缓存复用入口

## 问题

1. **写性能再提升的优先级**：下一步要继续追 SQLite，是继续优化 dictionary（缓存/批量）、还是进一步降低索引写放大？
2. **跨语言吞吐**：是否需要提供“批量 resolve / 批量返回字符串”的 API 来降低 binding 往返成本？

## 运行 Benchmark

```bash
cargo run --example bench_compare -p nervusdb-core --release
```
