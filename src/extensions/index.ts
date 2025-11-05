/**
 * NervusDB Extensions - Application Layer
 *
 * 应用层扩展，包含：
 * - 全文检索（Full-text Search）
 * - 空间索引（Spatial Index）
 * - 图算法（Graph Algorithms）
 * - 高级查询（Advanced Query）
 *   - 模式匹配（Pattern Matching）
 *   - 路径查找（Path Finding）
 *   - 聚合查询（Aggregation）
 *
 * 这一层是 TypeScript 实现独有的功能。
 */

// 全文检索
export * from './fulltext/index.js';

// 空间索引
export * from './spatial/geometry.js';
export * from './spatial/rtree.js';
export * from './spatial/spatialQuery.js';
export type * from './spatial/types.js';

// 图算法
export * from './algorithms/index.js';

// 高级查询
export * from './query/pattern/index.js';
export * from './query/path/variable.js';
export * from './query/path/astar.js';
export * from './query/path/bidirectional.js';
export * from './query/path/bidirectionalSimple.js';
export * from './query/aggregation.js';
