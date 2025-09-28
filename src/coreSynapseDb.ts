import { PersistentStore, FactInput, FactRecord } from './storage/persistentStore.js';
import { TripleKey } from './storage/propertyStore.js';
import {
  FactCriteria,
  FrontierOrientation,
  QueryBuilder,
  buildFindContext,
} from './query/queryBuilder.js';
import type {
  SynapseDBOpenOptions,
  CommitBatchOptions,
  BeginBatchOptions,
} from './types/openOptions.js';

export interface FactOptions {
  subjectProperties?: Record<string, unknown>;
  objectProperties?: Record<string, unknown>;
  edgeProperties?: Record<string, unknown>;
}

/**
 * CoreSynapseDB - 嵌入式三元组知识库核心版本
 *
 * 只包含核心功能：存储、查询、事务。
 * 所有高级功能（Cypher、路径查询、聚合等）都作为插件提供。
 *
 * "好品味"原则：专注做好一件事，没有特殊情况。
 */
export class CoreSynapseDB {
  protected constructor(private readonly store: PersistentStore) {}

  /**
   * 打开或创建 SynapseDB 数据库
   */
  static async open(path: string, options?: SynapseDBOpenOptions): Promise<CoreSynapseDB> {
    const store = await PersistentStore.open(path, options ?? {});
    return new CoreSynapseDB(store);
  }

  /**
   * 添加事实（三元组）
   */
  addFact(fact: FactInput, options: FactOptions = {}): FactRecord {
    const persisted = this.store.addFact(fact);

    if (options.subjectProperties) {
      this.store.setNodeProperties(persisted.subjectId, options.subjectProperties);
    }

    if (options.objectProperties) {
      this.store.setNodeProperties(persisted.objectId, options.objectProperties);
    }

    if (options.edgeProperties) {
      const tripleKey: TripleKey = {
        subjectId: persisted.subjectId,
        predicateId: persisted.predicateId,
        objectId: persisted.objectId,
      };
      this.store.setEdgeProperties(tripleKey, options.edgeProperties);
    }

    return {
      ...persisted,
      subjectProperties: this.store.getNodeProperties(persisted.subjectId),
      objectProperties: this.store.getNodeProperties(persisted.objectId),
      edgeProperties: this.store.getEdgeProperties({
        subjectId: persisted.subjectId,
        predicateId: persisted.predicateId,
        objectId: persisted.objectId,
      }),
    };
  }

  /**
   * 列出所有事实
   */
  listFacts(): FactRecord[] {
    return this.store.listFacts();
  }

  /**
   * 查询事实
   */
  find(criteria: FactCriteria, options?: { anchor?: FrontierOrientation }): QueryBuilder {
    const anchor = options?.anchor ?? this.inferAnchor(criteria);
    const context = buildFindContext(this.store, criteria, anchor);
    return QueryBuilder.fromFindResult(this.store, context);
  }

  /**
   * 删除事实
   */
  deleteFact(fact: FactInput): void {
    this.store.deleteFact(fact);
  }

  /**
   * 获取节点ID
   */
  getNodeId(value: string): number | undefined {
    return this.store.getNodeIdByValue(value);
  }

  /**
   * 获取节点值
   */
  getNodeValue(id: number): string | undefined {
    return this.store.getNodeValueById(id);
  }

  /**
   * 获取节点属性
   */
  getNodeProperties(nodeId: number): Record<string, unknown> | null {
    const v = this.store.getNodeProperties(nodeId);
    return v ?? null;
  }

  /**
   * 获取边属性
   */
  getEdgeProperties(key: TripleKey): Record<string, unknown> | null {
    const v = this.store.getEdgeProperties(key);
    return v ?? null;
  }

  /**
   * 设置节点属性
   */
  setNodeProperties(nodeId: number, properties: Record<string, unknown>): void {
    this.store.setNodeProperties(nodeId, properties);
  }

  /**
   * 设置边属性
   */
  setEdgeProperties(key: TripleKey, properties: Record<string, unknown>): void {
    this.store.setEdgeProperties(key, properties);
  }

  /**
   * 开始事务批次
   */
  beginBatch(options?: BeginBatchOptions): void {
    this.store.beginBatch(options);
  }

  /**
   * 提交事务批次
   */
  commitBatch(options?: CommitBatchOptions): void {
    this.store.commitBatch(options);
  }

  /**
   * 回滚事务批次
   */
  abortBatch(): void {
    this.store.abortBatch();
  }

  /**
   * 刷新到磁盘
   */
  async flush(): Promise<void> {
    await this.store.flush();
  }

  /**
   * 关闭数据库
   */
  async close(): Promise<void> {
    await this.store.close();
  }

  /**
   * 获取底层存储（供插件使用）
   */
  getStore(): PersistentStore {
    return this.store;
  }

  private inferAnchor(criteria: FactCriteria): FrontierOrientation {
    const hasSubject = criteria.subject !== undefined;
    const hasObject = criteria.object !== undefined;
    const hasPredicate = criteria.predicate !== undefined;

    if (hasSubject && hasObject) {
      return 'both';
    }
    if (hasSubject) {
      return 'subject';
    }
    if (hasObject && hasPredicate) {
      return 'subject';
    }
    if (hasObject) {
      return 'object';
    }
    return 'object';
  }
}

export type { FactInput, FactRecord, SynapseDBOpenOptions, CommitBatchOptions, BeginBatchOptions };
