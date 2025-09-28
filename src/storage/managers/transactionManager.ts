import { EncodedTriple } from '../tripleStore.js';
import { WalManager } from './walManager.js';

export interface TransactionBatch {
  adds: EncodedTriple[];
  dels: EncodedTriple[];
  nodeProps: Map<number, Record<string, unknown>>;
  edgeProps: Map<string, Record<string, unknown>>;
}

export interface BatchOptions {
  txId?: string;
  sessionId?: string;
}

export interface CommitOptions {
  durable?: boolean;
}

export interface TransactionContext {
  batchDepth: number;
  metaStack: Array<{ txId?: string; sessionId?: string }>;
  txStack: TransactionBatch[];
}

/**
 * 独立事务管理器：专门负责批次嵌套、暂存栈和事务语义
 *
 * 职责：
 * 1. 管理批次的开始/提交/回滚
 * 2. 维护嵌套事务栈
 * 3. 提供事务状态查询
 * 4. 与 WAL 协调记录事务操作
 */
export class TransactionManager {
  private batchDepth = 0;
  private batchMetaStack: Array<{ txId?: string; sessionId?: string }> = [];
  private txStack: TransactionBatch[] = [];

  constructor(private readonly wal: WalManager) {}

  /**
   * 开始一个新的事务批次
   */
  beginBatch(options?: BatchOptions): void {
    // 记录每一层的 BEGIN（含可选 tx 元信息），便于 WAL 重放时支持嵌套语义
    void this.wal.appendBegin(options);
    this.batchDepth += 1;
    this.batchMetaStack.push({ txId: options?.txId, sessionId: options?.sessionId });
    this.txStack.push({
      adds: [],
      dels: [],
      nodeProps: new Map(),
      edgeProps: new Map(),
    });
  }

  /**
   * 提交当前批次
   */
  commitBatch(options?: CommitOptions): TransactionBatch | null {
    if (this.batchDepth > 0) this.batchDepth -= 1;
    const stage = this.txStack.pop();

    // 将提交记录写入 WAL（内层也记录，以支持重放栈语义）
    if (options?.durable) void this.wal.appendCommitDurable();
    else void this.wal.appendCommit();

    // 记录已提交的 txId
    const meta = this.batchMetaStack.pop();
    if (meta?.txId) {
      void this.wal.recordCommittedTx([{ id: meta.txId, sessionId: meta.sessionId }]).catch(() => {
        /* ignore registry error */
      });
    }

    return stage || null;
  }

  /**
   * 回滚当前批次
   */
  abortBatch(): void {
    // 放弃当前顶层批次（仅一层），支持嵌套部分回滚
    if (this.batchDepth <= 0) return;
    this.batchDepth -= 1;
    void this.wal.appendAbort();
    // 丢弃当前层暂存与元信息
    this.batchMetaStack.pop();
    this.txStack.pop();
  }

  /**
   * 检查是否在批次中
   */
  isInBatch(): boolean {
    return this.batchDepth > 0;
  }

  /**
   * 获取当前批次深度
   */
  getBatchDepth(): number {
    return this.batchDepth;
  }

  /**
   * 检查是否是最外层批次
   */
  isOutermostBatch(): boolean {
    return this.batchDepth === 1;
  }

  /**
   * 获取当前事务的暂存数据
   */
  getCurrentBatch(): TransactionBatch | undefined {
    return this.txStack[this.txStack.length - 1];
  }

  /**
   * 添加事实到当前批次
   */
  addTripleToCurrentBatch(triple: EncodedTriple): void {
    const batch = this.getCurrentBatch();
    if (batch) {
      batch.adds.push(triple);
    }
  }

  /**
   * 添加删除操作到当前批次
   */
  deleteTripleFromCurrentBatch(triple: EncodedTriple): void {
    const batch = this.getCurrentBatch();
    if (batch) {
      batch.dels.push(triple);
    }
  }

  /**
   * 设置节点属性到当前批次
   */
  setNodePropertiesInCurrentBatch(nodeId: number, properties: Record<string, unknown>): void {
    const batch = this.getCurrentBatch();
    if (batch) {
      batch.nodeProps.set(nodeId, properties);
    }
  }

  /**
   * 设置边属性到当前批次
   */
  setEdgePropertiesInCurrentBatch(edgeKey: string, properties: Record<string, unknown>): void {
    const batch = this.getCurrentBatch();
    if (batch) {
      batch.edgeProps.set(edgeKey, properties);
    }
  }

  /**
   * 从事务栈中查找节点属性（支持嵌套覆盖）
   */
  getNodePropertiesFromTransaction(nodeId: number): Record<string, unknown> | undefined {
    // 从最新的事务向老的事务查找，支持嵌套覆盖
    for (let i = this.txStack.length - 1; i >= 0; i -= 1) {
      const value = this.txStack[i].nodeProps.get(nodeId);
      if (value !== undefined) return value;
    }
    return undefined;
  }

  /**
   * 从事务栈中查找边属性（支持嵌套覆盖）
   */
  getEdgePropertiesFromTransaction(edgeKey: string): Record<string, unknown> | undefined {
    // 从最新的事务向老的事务查找，支持嵌套覆盖
    for (let i = this.txStack.length - 1; i >= 0; i -= 1) {
      const value = this.txStack[i].edgeProps.get(edgeKey);
      if (value !== undefined) return value;
    }
    return undefined;
  }

  /**
   * 获取完整的事务上下文（用于调试或状态导出）
   */
  getTransactionContext(): TransactionContext {
    return {
      batchDepth: this.batchDepth,
      metaStack: [...this.batchMetaStack],
      txStack: this.txStack.map((batch) => ({
        adds: [...batch.adds],
        dels: [...batch.dels],
        nodeProps: new Map(batch.nodeProps),
        edgeProps: new Map(batch.edgeProps),
      })),
    };
  }

  /**
   * 清理所有事务状态（用于关闭或重置）
   */
  clear(): void {
    this.batchDepth = 0;
    this.batchMetaStack.length = 0;
    this.txStack.length = 0;
  }
}
