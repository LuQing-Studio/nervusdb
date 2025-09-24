/**
 * Gremlin GraphTraversalSource 适配器
 *
 * 提供与 Apache TinkerPop 兼容的图遍历入口点
 * 将 Gremlin 查询适配到 SynapseDB 存储引擎
 */

import type { PersistentStore } from '../../storage/persistentStore.js';
import type { Vertex, Edge, ElementId, TraversalConfig, TraversalStrategy } from './types.js';
import { GremlinError } from './types.js';
import { GraphTraversal } from './traversal.js';
import { GremlinExecutor } from './executor.js';

// 遍历源配置
interface TraversalSourceConfig {
  strategies?: TraversalStrategy[];
  sideEffects?: Map<string, unknown>;
  requirements?: Set<string>;
  // SynapseDB 特定配置
  batchSize?: number;
  enableOptimization?: boolean;
  cacheSize?: number;
}

// 遍历源统计
interface TraversalSourceStats {
  totalTraversals: number;
  avgExecutionTime: number;
  cacheHitRate: number;
  activeTraversals: number;
}

/**
 * GraphTraversalSource - Gremlin 图遍历源
 *
 * 作为所有图遍历的入口点，提供与 TinkerPop 兼容的 API
 */
export class GraphTraversalSource {
  private readonly store: PersistentStore;
  private readonly executor: GremlinExecutor;
  private config: TraversalSourceConfig;
  private stats: TraversalSourceStats;
  private executionCache: Map<string, any>;

  constructor(store: PersistentStore, config: TraversalSourceConfig = {}) {
    this.store = store;
    this.executor = new GremlinExecutor(store);
    this.config = {
      batchSize: 1000,
      enableOptimization: true,
      cacheSize: 100,
      ...config,
    };

    this.stats = {
      totalTraversals: 0,
      avgExecutionTime: 0,
      cacheHitRate: 0,
      activeTraversals: 0,
    };

    this.executionCache = new Map();
  }

  // ============ 遍历起点方法 ============

  /**
   * V() - 开始顶点遍历
   */
  V(...ids: ElementId[]): GraphTraversal<Vertex, Vertex> {
    return this.createTraversal().V(...ids);
  }

  /**
   * E() - 开始边遍历
   */
  E(...ids: ElementId[]): GraphTraversal<Edge, Edge> {
    return this.createTraversal().E(...ids);
  }

  /**
   * addV() - 添加顶点（暂不支持，SynapseDB 使用事实添加）
   */
  addV(label?: string): GraphTraversal<Vertex, Vertex> {
    throw new GremlinError('addV() 暂不支持，请使用 SynapseDB.addFact() 添加顶点');
  }

  /**
   * addE() - 添加边（暂不支持，SynapseDB 使用事实添加）
   */
  addE(label: string): GraphTraversal<Edge, Edge> {
    throw new GremlinError('addE() 暂不支持，请使用 SynapseDB.addFact() 添加边');
  }

  // ============ 配置方法 ============

  /**
   * withStrategies() - 添加遍历策略
   */
  withStrategies(...strategies: TraversalStrategy[]): GraphTraversalSource {
    const newConfig = {
      ...this.config,
      strategies: [...(this.config.strategies || []), ...strategies],
    };
    return new GraphTraversalSource(this.store, newConfig);
  }

  /**
   * withoutStrategies() - 移除遍历策略
   */
  withoutStrategies(...strategies: string[]): GraphTraversalSource {
    const newConfig = {
      ...this.config,
      strategies: (this.config.strategies || []).filter((s) => !strategies.includes(s.name)),
    };
    return new GraphTraversalSource(this.store, newConfig);
  }

  /**
   * withSideEffect() - 添加副作用
   */
  withSideEffect(key: string, value: unknown): GraphTraversalSource {
    const sideEffects = new Map(this.config.sideEffects);
    sideEffects.set(key, value);

    const newConfig = {
      ...this.config,
      sideEffects,
    };
    return new GraphTraversalSource(this.store, newConfig);
  }

  /**
   * withBulk() - 批量配置
   */
  withBulk(bulk: boolean): GraphTraversalSource {
    // SynapseDB 始终支持批量，此方法用于兼容性
    return this;
  }

  /**
   * withPath() - 路径跟踪配置
   */
  withPath(): GraphTraversalSource {
    const newConfig = {
      ...this.config,
      requirements: new Set([...(this.config.requirements || []), 'path']),
    };
    return new GraphTraversalSource(this.store, newConfig);
  }

  // ============ 克隆和配置 ============

  /**
   * clone() - 克隆遍历源
   */
  clone(): GraphTraversalSource {
    return new GraphTraversalSource(this.store, { ...this.config });
  }

  /**
   * getStrategies() - 获取当前策略
   */
  getStrategies(): TraversalStrategy[] {
    return [...(this.config.strategies || [])];
  }

  /**
   * getBytecode() - 获取字节码（暂不支持）
   */
  getBytecode(): any {
    throw new GremlinError('getBytecode() 在 SynapseDB 适配器中暂不支持');
  }

  // ============ 执行和管理 ============

  /**
   * close() - 关闭遍历源
   */
  async close(): Promise<void> {
    this.executionCache.clear();
    // SynapseDB store 由外部管理，不在此关闭
  }

  /**
   * getStats() - 获取统计信息
   */
  getStats(): TraversalSourceStats {
    return { ...this.stats };
  }

  /**
   * clearCache() - 清理执行缓存
   */
  clearCache(): void {
    this.executionCache.clear();
    this.stats.cacheHitRate = 0;
  }

  /**
   * warmUp() - 预热遍历源
   */
  async warmUp(): Promise<void> {
    // 预执行一些常见查询模式来预热
    try {
      const warmupTraversals = [this.V().limit(1), this.E().limit(1)];

      for (const traversal of warmupTraversals) {
        try {
          await traversal.hasNext();
        } catch {
          // 忽略预热错误
        }
      }
    } catch {
      // 忽略预热失败
    }
  }

  // ============ 内部工具方法 ============

  /**
   * 创建新的遍历实例
   */
  private createTraversal<S = Vertex | Edge, E = Vertex | Edge>(): GraphTraversal<S, E> {
    const traversalConfig: TraversalConfig = {
      strategies: this.config.strategies,
      sideEffects: this.config.sideEffects,
      requirements: this.config.requirements,
      batch: this.config.batchSize
        ? {
            batchSize: this.config.batchSize,
            timeout: 30000,
            concurrent: true,
          }
        : undefined,
    };

    return new GraphTraversal<S, E>(this.store, [], traversalConfig);
  }

  /**
   * 更新执行统计
   */
  private updateStats(executionTime: number, cacheHit: boolean): void {
    this.stats.totalTraversals++;

    // 更新平均执行时间
    const total = this.stats.totalTraversals;
    this.stats.avgExecutionTime =
      (this.stats.avgExecutionTime * (total - 1) + executionTime) / total;

    // 更新缓存命中率
    if (cacheHit) {
      this.stats.cacheHitRate = (this.stats.cacheHitRate * (total - 1) + 1) / total;
    } else {
      this.stats.cacheHitRate = (this.stats.cacheHitRate * (total - 1)) / total;
    }
  }

  /**
   * 生成缓存键
   */
  private generateCacheKey(steps: any[]): string {
    return JSON.stringify(
      steps.map((step) => ({
        type: step.type,
        ...step,
      })),
    );
  }
}

/**
 * 工厂函数：创建 GraphTraversalSource
 */
export function createTraversalSource(
  store: PersistentStore,
  config?: TraversalSourceConfig,
): GraphTraversalSource {
  return new GraphTraversalSource(store, config);
}

/**
 * 便捷方法：从 SynapseDB 创建遍历源
 */
export function traversal(store: PersistentStore): GraphTraversalSource {
  return createTraversalSource(store);
}
