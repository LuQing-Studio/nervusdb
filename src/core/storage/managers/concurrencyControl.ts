import { addReader, removeReader } from '../readerRegistry.js';
import { acquireLock, type LockHandle } from '../../../utils/lock.js';

export interface ReaderInfo {
  pid: number;
  epoch: number;
  ts: number;
}

export interface ConcurrencyOptions {
  enableLock?: boolean;
  registerReader?: boolean;
}

/**
 * 并发控制管理器：专门负责锁管理、读者注册和epoch固定
 *
 * 职责：
 * 1. 进程级文件锁管理
 * 2. 读者注册与注销
 * 3. Epoch 固定与快照一致性
 * 4. 读者可见性管理
 */
export class ConcurrencyControl {
  private lock?: LockHandle;
  private currentEpoch = 0;
  private lastManifestCheck = 0;
  private pinnedEpochStack: number[] = [];
  private readerRegistered = false;
  private snapshotRefCount = 0;
  private activeReaderOperation: Promise<void> | null = null;

  constructor(
    private readonly indexDirectory: string,
    private readonly path: string,
  ) {}

  /**
   * 获取进程级独占写锁
   */
  async acquireWriteLock(): Promise<void> {
    if (!this.lock) {
      this.lock = await acquireLock(this.path);
    }
  }

  /**
   * 释放写锁
   */
  async releaseWriteLock(): Promise<void> {
    if (this.lock) {
      await this.lock.release();
      this.lock = undefined;
    }
  }

  /**
   * 检查是否持有写锁
   */
  hasWriteLock(): boolean {
    return this.lock !== undefined;
  }

  /**
   * 设置当前epoch
   */
  setCurrentEpoch(epoch: number): void {
    this.currentEpoch = epoch;
  }

  /**
   * 获取当前epoch
   */
  getCurrentEpoch(): number {
    return this.currentEpoch;
  }

  /**
   * 获取固定的epoch栈
   */
  getPinnedEpochStack(): readonly number[] {
    return this.pinnedEpochStack;
  }

  /**
   * 检查是否有固定的epoch
   */
  hasPinnedEpoch(): boolean {
    return this.pinnedEpochStack.length > 0;
  }

  /**
   * 获取最近的manifest检查时间
   */
  getLastManifestCheck(): number {
    return this.lastManifestCheck;
  }

  /**
   * 更新manifest检查时间
   */
  updateManifestCheck(): void {
    this.lastManifestCheck = Date.now();
  }

  /**
   * 检查是否需要刷新readers（节流检查）
   */
  shouldRefreshReaders(): boolean {
    const now = Date.now();
    return this.pinnedEpochStack.length === 0 && now - this.lastManifestCheck > 1000;
  }

  /**
   * 确保读者注册的异步锁机制
   */
  private async ensureReaderRegistered(epoch: number): Promise<void> {
    // 如果已有操作在进行中，等待其完成
    if (this.activeReaderOperation) {
      await this.activeReaderOperation;
      return;
    }

    // 如果已经注册过读者，无需重复注册
    if (this.readerRegistered) {
      return;
    }

    // 启动新的注册操作
    this.activeReaderOperation = (async () => {
      try {
        await addReader(this.indexDirectory, {
          pid: process.pid,
          epoch: epoch,
          ts: Date.now(),
        });
        this.readerRegistered = true;
      } catch {
        // 注册失败，保持标志位为false
        this.readerRegistered = false;
      }
    })();

    await this.activeReaderOperation;
    this.activeReaderOperation = null;
  }

  /**
   * 注册读者（在数据库打开时）
   */
  async registerReader(epoch: number): Promise<void> {
    try {
      await addReader(this.indexDirectory, {
        pid: process.pid,
        epoch: epoch,
        ts: Date.now(),
      });
      this.readerRegistered = true;
    } catch {
      this.readerRegistered = false;
      throw new Error('Failed to register reader');
    }
  }

  /**
   * 注销读者
   */
  async unregisterReader(): Promise<void> {
    if (this.readerRegistered) {
      try {
        await removeReader(this.indexDirectory, process.pid);
      } catch {
        // ignore registry errors
      }
      this.readerRegistered = false;
    }
  }

  /**
   * 检查是否已注册读者
   */
  isReaderRegistered(): boolean {
    return this.readerRegistered;
  }

  /**
   * 读一致性：在查询链路中临时固定 epoch，避免中途重载 readers
   */
  async pushPinnedEpoch(epoch: number): Promise<void> {
    this.pinnedEpochStack.push(epoch);
    this.snapshotRefCount++;

    // 如果这是第一个快照，确保读者已注册
    if (this.snapshotRefCount === 1) {
      await this.ensureReaderRegistered(epoch);
    }
  }

  /**
   * 释放固定的epoch
   */
  async popPinnedEpoch(): Promise<void> {
    this.pinnedEpochStack.pop();
    this.snapshotRefCount--;

    // 如果这是最后一个快照，且之前注册过读者，则注销
    if (this.snapshotRefCount === 0 && this.readerRegistered) {
      try {
        await removeReader(this.indexDirectory, process.pid);
        this.readerRegistered = false;
      } catch {
        // 忽略注销失败，但不保证readerRegistered状态
      }
    }
  }

  /**
   * 获取快照引用计数
   */
  getSnapshotRefCount(): number {
    return this.snapshotRefCount;
  }

  /**
   * 清理所有并发控制状态（用于关闭或重置）
   */
  async cleanup(): Promise<void> {
    // 释放写锁
    await this.releaseWriteLock();

    // 注销读者
    await this.unregisterReader();

    // 清理状态
    this.pinnedEpochStack.length = 0;
    this.snapshotRefCount = 0;
    this.activeReaderOperation = null;
    this.lastManifestCheck = 0;
  }

  /**
   * 获取并发控制状态（用于调试）
   */
  getState() {
    return {
      hasWriteLock: this.hasWriteLock(),
      isReaderRegistered: this.isReaderRegistered(),
      currentEpoch: this.currentEpoch,
      pinnedEpochStack: [...this.pinnedEpochStack],
      snapshotRefCount: this.snapshotRefCount,
      lastManifestCheck: this.lastManifestCheck,
    };
  }
}
