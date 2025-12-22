# T2: 清理 Node 侧 `.synapsedb/.pages` 遗留（归档/删除）

## 1. Context

- 现状：`bindings/node/src` 与测试体系仍大量出现 `.synapsedb/.pages/.wal` 等旧存储概念，但 v2 Rust Core 已经是 `redb` 单文件。
- 这会把代码路径变成迷宫：一边宣称单文件，一边又维护一套“分页索引 + WAL + 维护工具”的旧世界。

## 2. Goals

- `bindings/node/src` 不再包含 `.pages` / paged-index / WAL / 维护工具（compaction/gc/repair）等旧引擎概念与代码。
- Node 包默认只对接 Rust Native（`redb` 单文件），并且文档/示例不再鼓励使用 `.synapsedb` 作为主路径扩展名。
- 迁移严格手动：只提供 `nervus-migrate`，`open()` 不做任何自动迁移与探测。

## 3. Non-Goals

- 不保留“显式 Legacy 引擎”（不做双引擎、也不做自动 fallback）。
- 不在 `open()` 里加入格式检测/自动迁移/自动修复。

## 4. Solution

1. 删除或移入 `_archive/`：
   - Node 侧与 `.pages` 相关的实现、维护工具与系统测试（包括 crash injection、compaction、gc、repair、wal 语义等）。
2. 收口 `NervusDBOpenOptions`：
   - 移除旧引擎专用字段（如 `indexDirectory/pageSize/rebuildIndexes/compression/...`）。
   - 保留的选项必须能映射到 Rust Core 的真实行为，否则就别存在。
3. 统一路径口径：
   - Node/文档/bench 默认示例改为 `*.redb`（或无扩展名 base path，但最终只生成一个 `*.redb` 文件）。

## 5. Testing Strategy

- `bindings/node`：保留并补齐“native wrapper + mock binding”的单元测试。
- 增加最小端到端：创建 DB → 写入 → 查询 → 关闭 → 重开 → 校验结果。

## 6. Risks

- 这是 breaking change（但 Node 包当前是 `0.x`，允许快速清理；发布时要在 CHANGELOG 明确写清楚）。

