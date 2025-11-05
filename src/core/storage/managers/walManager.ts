import { WalReplayer, WalWriter, type WalBeginMeta } from '../wal.js';
import type { FactInput } from '../types.js';
import {
  readTxIdRegistry,
  writeTxIdRegistry,
  mergeTxIds,
  toSet,
  type TxIdRegistryData,
} from '../txidRegistry.js';

export interface WalManagerOptions {
  indexDirectory: string;
  enablePersistentTxDedupe?: boolean;
  maxRememberTxIds?: number;
}

export type WalReplaySnapshot = Awaited<ReturnType<WalReplayer['replay']>>;

const DEFAULT_MAX_TX_IDS = 1000;

/**
 * WAL 管理器：封装重放、去重与持久化 txId 注册表。
 */
export class WalManager {
  private constructor(
    private readonly dbPath: string,
    private readonly writer: WalWriter,
    private registry: TxIdRegistryData,
    private readonly enablePersistentTx: boolean,
    private readonly indexDirectory: string,
    private readonly maxRememberTxIds: number,
  ) {}

  static async initialize(
    dbPath: string,
    options: WalManagerOptions,
  ): Promise<{ manager: WalManager; replay: WalReplaySnapshot }> {
    const persistentTx = options.enablePersistentTxDedupe === true;
    const maxTx = options.maxRememberTxIds ?? DEFAULT_MAX_TX_IDS;

    const writer = await WalWriter.open(dbPath);
    let registry: TxIdRegistryData = { version: 1, txIds: [] };
    if (persistentTx) {
      registry = await readTxIdRegistry(options.indexDirectory);
    }

    const known = persistentTx ? toSet(registry) : undefined;
    const replay = await new WalReplayer(dbPath).replay(known);

    if (replay.safeOffset > 0) {
      await writer.truncateTo(replay.safeOffset);
    }

    if (persistentTx && replay.committedTx.length > 0) {
      registry = mergeTxIds(registry, replay.committedTx, maxTx);
      await writeTxIdRegistry(options.indexDirectory, registry);
    }

    const manager = new WalManager(
      dbPath,
      writer,
      registry,
      persistentTx,
      options.indexDirectory,
      maxTx,
    );

    return { manager, replay };
  }

  appendAddTriple(fact: FactInput): void {
    this.writer.appendAddTriple(fact);
  }

  appendDeleteTriple(fact: FactInput): void {
    this.writer.appendDeleteTriple(fact);
  }

  appendSetNodeProps(nodeId: number, props: unknown): void {
    this.writer.appendSetNodeProps(nodeId, props);
  }

  appendSetEdgeProps(
    ids: { subjectId: number; predicateId: number; objectId: number },
    props: unknown,
  ): void {
    this.writer.appendSetEdgeProps(ids, props);
  }

  appendBegin(meta?: WalBeginMeta): void {
    this.writer.appendBegin(meta);
  }

  appendCommit(): void {
    this.writer.appendCommit();
  }

  async appendCommitDurable(): Promise<void> {
    await this.writer.appendCommitDurable();
  }

  appendAbort(): void {
    this.writer.appendAbort();
  }

  async reset(): Promise<void> {
    await this.writer.reset();
  }

  async truncateTo(offset: number): Promise<void> {
    await this.writer.truncateTo(offset);
  }

  async recordCommittedTx(entries: Array<{ id: string; sessionId?: string }>): Promise<void> {
    if (!this.enablePersistentTx || entries.length === 0) {
      return;
    }
    this.registry = mergeTxIds(this.registry, entries, this.maxRememberTxIds);
    await writeTxIdRegistry(this.indexDirectory, this.registry);
  }

  isPersistentTxEnabled(): boolean {
    return this.enablePersistentTx;
  }

  async close(): Promise<void> {
    await this.writer.close();
  }
}
