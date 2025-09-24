/**
 * Gremlin 图遍历语言支持
 *
 * 为 SynapseDB 提供兼容 Apache TinkerPop 的 Gremlin 遍历 API
 * 支持链式遍历、复杂过滤和聚合查询
 */

import type { PersistentStore } from '../../storage/persistentStore.js';
import { GraphTraversalSource, createTraversalSource, traversal } from './source.js';
import { GraphTraversal } from './traversal.js';
import { GremlinExecutor } from './executor.js';

// 导出核心类
export { GraphTraversalSource, GraphTraversal, GremlinExecutor };

// 导出工厂函数
export { createTraversalSource, traversal };

// 导出类型定义
export type {
  // 基础图元素
  Vertex,
  Edge,
  Element,
  Path,
  Traverser,

  // 谓词和操作符
  Predicate,

  // 枚举类型
  Direction,
  Scope,
  Column,
  Order,
  Cardinality,

  // 配置和策略
  TraversalStrategy,
  TraversalConfig,
  SideEffect,
  BatchConfig,

  // 工具类型
  ElementId,
  PropertyKey,
  PropertyValue,
  Label,

  // 错误类型
  GremlinError,
  UnsupportedStepError,
  TraversalError,

  // 统计和度量
  TraversalMetrics,
  TraversalExplanation,
  GraphTraversalStats,
  SubGraph,
} from './types.js';

// 导出步骤类型（用于高级用法）
export type {
  GremlinStep,
  Step,
  VStep,
  EStep,
  HasStep,
  OutStep,
  InStep,
  // ... 其他步骤类型按需导出
} from './step.js';

// 导出遍历结果类型
export type { TraversalResult } from './traversal.js';

/**
 * 便捷方法：为 SynapseDB 添加 Gremlin 支持
 *
 * @param store SynapseDB 持久化存储
 * @returns GraphTraversalSource 实例
 *
 * @example
 * ```typescript
 * import { SynapseDB } from '../synapseDb';
 * import { gremlin } from './query/gremlin';
 *
 * const db = await SynapseDB.open('graph.synapsedb');
 * const g = gremlin(db.store);
 *
 * // 查找所有名为 "张三" 的人
 * const results = await g.V()
 *   .has('name', '张三')
 *   .toList();
 *
 * // 查找张三的朋友
 * const friends = await g.V()
 *   .has('name', '张三')
 *   .out('朋友')
 *   .values('name')
 *   .toList();
 * ```
 */
export function gremlin(store: PersistentStore): GraphTraversalSource {
  return traversal(store);
}

/**
 * 高级用法：创建配置化的 Gremlin 遍历源
 *
 * @param store SynapseDB 持久化存储
 * @param options 遍历源配置选项
 * @returns 配置化的 GraphTraversalSource 实例
 *
 * @example
 * ```typescript
 * import { SynapseDB } from '../synapseDb';
 * import { createGremlinSource } from './query/gremlin';
 *
 * const db = await SynapseDB.open('graph.synapsedb');
 * const g = createGremlinSource(db.store, {
 *   batchSize: 2000,
 *   enableOptimization: true,
 *   strategies: [
 *     { name: 'SubgraphStrategy', configuration: { vertices: ['Person'] } }
 *   ]
 * });
 *
 * // 使用优化策略执行复杂查询
 * const results = await g.V()
 *   .hasLabel('Person')
 *   .repeat(g.V().out('knows'))
 *   .times(3)
 *   .dedup()
 *   .toList();
 * ```
 */
export function createGremlinSource(
  store: PersistentStore,
  options?: {
    batchSize?: number;
    enableOptimization?: boolean;
    cacheSize?: number;
    strategies?: any[];
    sideEffects?: Map<string, unknown>;
  },
): GraphTraversalSource {
  return createTraversalSource(store, options);
}

// 常用谓词工厂函数
export const P = {
  /**
   * 等于
   */
  eq: (value: unknown) => ({ operator: 'eq' as const, value }),

  /**
   * 不等于
   */
  neq: (value: unknown) => ({ operator: 'neq' as const, value }),

  /**
   * 小于
   */
  lt: (value: unknown) => ({ operator: 'lt' as const, value }),

  /**
   * 小于等于
   */
  lte: (value: unknown) => ({ operator: 'lte' as const, value }),

  /**
   * 大于
   */
  gt: (value: unknown) => ({ operator: 'gt' as const, value }),

  /**
   * 大于等于
   */
  gte: (value: unknown) => ({ operator: 'gte' as const, value }),

  /**
   * 在范围内（开区间）
   */
  inside: (lower: unknown, upper: unknown) => ({
    operator: 'inside' as const,
    value: lower,
    other: upper,
  }),

  /**
   * 在范围外
   */
  outside: (lower: unknown, upper: unknown) => ({
    operator: 'outside' as const,
    value: lower,
    other: upper,
  }),

  /**
   * 在范围内（闭区间）
   */
  between: (lower: unknown, upper: unknown) => ({
    operator: 'between' as const,
    value: lower,
    other: upper,
  }),

  /**
   * 在列表中
   */
  within: (...values: unknown[]) => ({
    operator: 'within' as const,
    value: values,
  }),

  /**
   * 不在列表中
   */
  without: (...values: unknown[]) => ({
    operator: 'without' as const,
    value: values,
  }),

  /**
   * 以指定值开头
   */
  startingWith: (prefix: string) => ({
    operator: 'startingWith' as const,
    value: prefix,
  }),

  /**
   * 以指定值结尾
   */
  endingWith: (suffix: string) => ({
    operator: 'endingWith' as const,
    value: suffix,
  }),

  /**
   * 包含指定值
   */
  containing: (substring: string) => ({
    operator: 'containing' as const,
    value: substring,
  }),

  /**
   * 不以指定值开头
   */
  notStartingWith: (prefix: string) => ({
    operator: 'notStartingWith' as const,
    value: prefix,
  }),

  /**
   * 不以指定值结尾
   */
  notEndingWith: (suffix: string) => ({
    operator: 'notEndingWith' as const,
    value: suffix,
  }),

  /**
   * 不包含指定值
   */
  notContaining: (substring: string) => ({
    operator: 'notContaining' as const,
    value: substring,
  }),
} as const;

/**
 * Gremlin 模块版本信息
 */
export const GREMLIN_VERSION = '1.0.0' as const;

/**
 * 与 TinkerPop 的兼容性版本
 */
export const TINKERPOP_VERSION = '3.7.0' as const;
