# T3: 重写 Rust interning（真 LRU）

## 1. Context

- `nervusdb-core/src/storage/disk.rs` 的 `WriteTableHandles` 目前用 `HashMap` 假装 LRU：`remove()+insert()` 在 `HashMap` 上没有“最近使用顺序”，这就是自欺欺人。
- Intern 是写路径的热点：每条 fact 至少 3 次（S/P/O），缓存做错了等于没做。

## 2. Goals

- 使用成熟的小依赖：`lru` crate。
- 在不改变语义的前提下，把 `WriteTableHandles` 的缓存变成真正的 LRU：
  - 命中 O(1) 且更新最近使用顺序。
  - 超过容量时严格淘汰最久未使用项。
- 不引入并发/全局缓存：缓存只存在于单个写事务上下文（`WriteTableHandles`），保持简单、可预测。

## 3. Non-Goals

- 不做跨事务的全局 LRU（那是另一种复杂度/一致性问题）。
- 不改变字典表结构（`str_to_id`/`id_to_str` 仍以 `redb` 表为准）。

## 4. Solution

- `WriteTableHandles.string_cache`：
  - `HashMap<String, u64>` → `lru::LruCache<String, u64>`（容量 `NonZeroUsize`）。
- `intern(value)` 流程：
  1. `cache.get(value)` 命中直接返回（LRU 自动更新）。
  2. 未命中：查 `str_to_id.get(value)`；若存在则写入 cache 并返回。
  3. 不存在：分配新 `id`，写入两张表，并写入 cache。
- 保留 `fast_intern`（空库快速路径）与 `next_id` 逻辑，避免每次 `last()`。

## 5. Testing Strategy

- Rust 单测：
  - 重复写入同一批字符串：字典 ID 稳定、`dictionary_size` 符合预期。
  - 压力场景：大量重复值 + 少量新值，确保不会触发错误映射（正确性优先于缓存命中率）。
- 基准回归：
  - `nervusdb-core/examples/bench_compare.rs`：只接受“无倒退”（写入/查询不下降）。

## 6. Risks

- 依赖引入：需要锁定版本并写入 `Cargo.toml`（可控）。
- 内存占用：LRU 容量上限必须明确且保守（现有 `STRING_CACHE_LIMIT=100_000` 可沿用）。

