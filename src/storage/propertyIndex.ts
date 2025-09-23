/**
 * 属性倒排索引 - 支持基于属性值的高效查询
 *
 * 设计目标：
 * - 支持 O(log n) 时间复杂度的属性值查询
 * - 支持范围查询和等值查询
 * - 内存友好的分页存储
 * - 支持增量更新和批量重建
 */

import * as fs from 'node:fs/promises';
import * as path from 'node:path';

// 索引条目：属性名 -> 值 -> ID集合
export interface PropertyIndexEntry {
  propertyName: string;
  value: unknown;
  nodeIds?: Set<number>; // 节点属性索引
  edgeKeys?: Set<string>; // 边属性索引
}

// 属性索引操作类型
export type PropertyOperation = 'SET' | 'DELETE';

export interface PropertyChange {
  operation: PropertyOperation;
  target: 'node' | 'edge';
  targetId: number | string;
  propertyName: string;
  oldValue?: unknown;
  newValue?: unknown;
}

/**
 * 内存属性索引 - 暂存层，支持快速查询和更新
 */
export class MemoryPropertyIndex {
  // nodeProperties: 属性名 -> 归一化值 -> nodeId集合
  private readonly nodeProperties = new Map<
    string,
    Map<string | number | boolean | null, Set<number>>
  >();

  // edgeProperties: 属性名 -> 归一化值 -> edgeKey集合
  private readonly edgeProperties = new Map<
    string,
    Map<string | number | boolean | null, Set<string>>
  >();

  /**
   * 添加节点属性到索引
   */
  indexNodeProperty(nodeId: number, propertyName: string, value: unknown): void {
    if (!this.nodeProperties.has(propertyName)) {
      this.nodeProperties.set(propertyName, new Map());
    }

    const valueMap = this.nodeProperties.get(propertyName)!;
    const key = this.normalizeValue(value);

    if (!valueMap.has(key)) {
      valueMap.set(key, new Set());
    }

    valueMap.get(key)!.add(nodeId);
  }

  /**
   * 添加边属性到索引
   */
  indexEdgeProperty(edgeKey: string, propertyName: string, value: unknown): void {
    if (!this.edgeProperties.has(propertyName)) {
      this.edgeProperties.set(propertyName, new Map());
    }

    const valueMap = this.edgeProperties.get(propertyName)!;
    const key = this.normalizeValue(value);

    if (!valueMap.has(key)) {
      valueMap.set(key, new Set());
    }

    valueMap.get(key)!.add(edgeKey);
  }

  /**
   * 从索引中移除节点属性
   */
  removeNodeProperty(nodeId: number, propertyName: string, value: unknown): void {
    const valueMap = this.nodeProperties.get(propertyName);
    if (!valueMap) return;

    const key = this.normalizeValue(value);
    const nodeSet = valueMap.get(key);
    if (!nodeSet) return;

    nodeSet.delete(nodeId);
    if (nodeSet.size === 0) {
      valueMap.delete(key);
      if (valueMap.size === 0) {
        this.nodeProperties.delete(propertyName);
      }
    }
  }

  /**
   * 从索引中移除边属性
   */
  removeEdgeProperty(edgeKey: string, propertyName: string, value: unknown): void {
    const valueMap = this.edgeProperties.get(propertyName);
    if (!valueMap) return;

    const key = this.normalizeValue(value);
    const edgeSet = valueMap.get(key);
    if (!edgeSet) return;

    edgeSet.delete(edgeKey);
    if (edgeSet.size === 0) {
      valueMap.delete(key);
      if (valueMap.size === 0) {
        this.edgeProperties.delete(propertyName);
      }
    }
  }

  /**
   * 查询具有指定属性值的节点ID
   */
  queryNodesByProperty(propertyName: string, value: unknown): Set<number> {
    const valueMap = this.nodeProperties.get(propertyName);
    if (!valueMap) return new Set();

    const key = this.normalizeValue(value);
    return new Set(valueMap.get(key) || []);
  }

  /**
   * 查询具有指定属性值的边键
   */
  queryEdgesByProperty(propertyName: string, value: unknown): Set<string> {
    const valueMap = this.edgeProperties.get(propertyName);
    if (!valueMap) return new Set();

    const key = this.normalizeValue(value);
    return new Set(valueMap.get(key) || []);
  }

  /**
   * 范围查询节点 (用于数值比较)
   */
  queryNodesByRange(
    propertyName: string,
    min?: unknown,
    max?: unknown,
    includeMin = true,
    includeMax = true,
  ): Set<number> {
    const valueMap = this.nodeProperties.get(propertyName);
    if (!valueMap) return new Set();

    const results = new Set<number>();

    for (const [value, nodeIds] of valueMap.entries()) {
      if (this.isInRange(value, min, max, includeMin, includeMax)) {
        for (const nodeId of nodeIds) {
          results.add(nodeId);
        }
      }
    }

    return results;
  }

  /**
   * 获取所有属性名
   */
  getNodePropertyNames(): string[] {
    return Array.from(this.nodeProperties.keys());
  }

  getEdgePropertyNames(): string[] {
    return Array.from(this.edgeProperties.keys());
  }

  /**
   * 获取统计信息
   */
  getStats(): {
    nodePropertyCount: number;
    edgePropertyCount: number;
    totalNodeEntries: number;
    totalEdgeEntries: number;
  } {
    let totalNodeEntries = 0;
    let totalEdgeEntries = 0;

    for (const valueMap of this.nodeProperties.values()) {
      for (const nodeSet of valueMap.values()) {
        totalNodeEntries += nodeSet.size;
      }
    }

    for (const valueMap of this.edgeProperties.values()) {
      for (const edgeSet of valueMap.values()) {
        totalEdgeEntries += edgeSet.size;
      }
    }

    return {
      nodePropertyCount: this.nodeProperties.size,
      edgePropertyCount: this.edgeProperties.size,
      totalNodeEntries,
      totalEdgeEntries,
    };
  }

  /**
   * 清空索引
   */
  clear(): void {
    this.nodeProperties.clear();
    this.edgeProperties.clear();
  }

  /**
   * 标准化值用于索引键
   */
  private normalizeValue(value: unknown): string | number | boolean | null {
    if (value === null || value === undefined) {
      return null;
    }

    // 对于对象和数组，使用 JSON 序列化作为键
    if (typeof value === 'object') {
      return JSON.stringify(value);
    }

    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
      return value;
    }
    if (typeof value === 'bigint') return value.toString();
    if (typeof value === 'symbol') return value.toString();
    if (typeof value === 'function') return '[function]';
    // 其余非常规类型统一序列化
    return JSON.stringify(value);
  }

  /**
   * 检查值是否在指定范围内
   */
  private isInRange(
    value: string | number | boolean | null,
    min?: unknown,
    max?: unknown,
    includeMin = true,
    includeMax = true,
  ): boolean {
    if (min !== undefined) {
      const cmp = this.compareValues(value, min);
      if (cmp < 0 || (cmp === 0 && !includeMin)) {
        return false;
      }
    }

    if (max !== undefined) {
      const cmp = this.compareValues(value, max);
      if (cmp > 0 || (cmp === 0 && !includeMax)) {
        return false;
      }
    }

    return true;
  }

  /**
   * 比较两个值
   */
  private compareValues(a: unknown, b: unknown): number {
    if (a === b) return 0;
    if (a == null && b != null) return -1;
    if (a != null && b == null) return 1;

    // 数值比较
    if (typeof a === 'number' && typeof b === 'number') {
      return a - b;
    }

    // 字符串比较
    if (typeof a === 'string' && typeof b === 'string') {
      return a.localeCompare(b);
    }

    // 日期比较
    if (a instanceof Date && b instanceof Date) {
      return a.getTime() - b.getTime();
    }

    // 其他类型转换为字符串比较
    return String(a).localeCompare(String(b));
  }
}

// 预留：分页属性索引清单（未来持久化时使用）

/**
 * 持久化属性索引管理器
 */
export class PropertyIndexManager {
  private readonly memoryIndex = new MemoryPropertyIndex();
  private readonly indexDirectory: string;
  private readonly manifestPath: string;

  constructor(indexDirectory: string) {
    this.indexDirectory = indexDirectory;
    this.manifestPath = path.join(indexDirectory, 'property-index.manifest.json');
  }

  /**
   * 获取内存索引实例
   */
  getMemoryIndex(): MemoryPropertyIndex {
    return this.memoryIndex;
  }

  /**
   * 初始化属性索引目录
   */
  async initialize(): Promise<void> {
    try {
      await fs.mkdir(this.indexDirectory, { recursive: true });
    } catch {}
  }

  /**
   * 从现有属性数据重建索引
   */
  async rebuildFromProperties(
    nodeProperties: Map<number, Record<string, unknown>>,
    edgeProperties: Map<string, Record<string, unknown>>,
  ): Promise<void> {
    // 预留异步点，后续引入磁盘持久化时会包含 IO
    await Promise.resolve();
    this.memoryIndex.clear();

    // 重建节点属性索引
    for (const [nodeId, props] of nodeProperties.entries()) {
      for (const [propName, value] of Object.entries(props)) {
        this.memoryIndex.indexNodeProperty(nodeId, propName, value);
      }
    }

    // 重建边属性索引
    for (const [edgeKey, props] of edgeProperties.entries()) {
      for (const [propName, value] of Object.entries(props)) {
        this.memoryIndex.indexEdgeProperty(edgeKey, propName, value);
      }
    }
  }

  /**
   * 处理属性变更
   */
  applyPropertyChange(change: PropertyChange): void {
    if (change.target === 'node') {
      const nodeId = change.targetId as number;

      if (change.operation === 'DELETE' && change.oldValue !== undefined) {
        this.memoryIndex.removeNodeProperty(nodeId, change.propertyName, change.oldValue);
      } else if (change.operation === 'SET') {
        // 先删除旧值（如果存在）
        if (change.oldValue !== undefined) {
          this.memoryIndex.removeNodeProperty(nodeId, change.propertyName, change.oldValue);
        }
        // 添加新值
        if (change.newValue !== undefined) {
          this.memoryIndex.indexNodeProperty(nodeId, change.propertyName, change.newValue);
        }
      }
    } else if (change.target === 'edge') {
      const edgeKey = change.targetId as string;

      if (change.operation === 'DELETE' && change.oldValue !== undefined) {
        this.memoryIndex.removeEdgeProperty(edgeKey, change.propertyName, change.oldValue);
      } else if (change.operation === 'SET') {
        // 先删除旧值（如果存在）
        if (change.oldValue !== undefined) {
          this.memoryIndex.removeEdgeProperty(edgeKey, change.propertyName, change.oldValue);
        }
        // 添加新值
        if (change.newValue !== undefined) {
          this.memoryIndex.indexEdgeProperty(edgeKey, change.propertyName, change.newValue);
        }
      }
    }
  }

  /**
   * 持久化索引到磁盘（未来实现）
   */
  async flush(): Promise<void> {
    // TODO: 实现分页 B+树持久化
    // 当前版本仅支持内存索引
  }

  /**
   * 从磁盘加载索引（未来实现）
   */
  async load(): Promise<void> {
    // TODO: 实现从分页文件加载
    // 当前版本仅支持内存索引
  }
}
