/**
 * NervusDB 数据库打开选项
 *
 * v2 仅使用 Rust Core（redb 单文件）作为存储引擎。
 *
 * 这里的选项只保留“能真实映射到 Rust Core 行为”的最小集合。
 */
export interface NervusDBOpenOptions {
  /**
   * 启用进程级独占写锁
   *
   * 当启用时，同一路径只允许一个写者进程访问。防止多个进程同时写入导致的数据损坏。
   * 读者不受此锁限制。
   *
   * @default false
   * @recommended true（生产环境）
   * @warning 禁用锁可能导致并发写入时的数据损坏
   */
  enableLock?: boolean;

  /**
   * 注册为读者
   *
   * 当启用时，此实例将在读者注册表中注册，允许维护任务（压缩、GC）
   * 检测活跃读者并避免影响正在进行的查询。
   *
   * @default true（自 v2 起）
   * @note 设为 false 可能导致维护任务与查询冲突
   */
  registerReader?: boolean;

  /**
   * 实验性功能开关
   *
   * 这些能力尚未稳定，默认关闭。建议仅在评估阶段显式开启。
   * 可通过环境变量 `SYNAPSEDB_ENABLE_EXPERIMENTAL_QUERIES=1` 全局开启查询语言前端。
   */
  experimental?: {
    /** 是否启用 Cypher 查询语言插件 */
    cypher?: boolean;
    /** 是否启用 Gremlin 查询语言辅助工厂 */
    gremlin?: boolean;
    /** 是否启用 GraphQL 查询语言辅助工厂 */
    graphql?: boolean;
  };
}

/**
 * 批次提交选项
 */
export interface CommitBatchOptions {
  /**
   * 持久性保证
   *
   * 当设为 true 时，提交操作将强制同步到磁盘（fsync），
   * 确保在系统崩溃后数据不会丢失。
   *
   * @default false
   * @note 启用会显著降低写入性能但提供更强的持久性保证
   */
  durable?: boolean;
}

/**
 * 批次开始选项
 */
export interface BeginBatchOptions {
  /**
   * 事务 ID
   *
   * 可选的事务标识符，用于幂等性控制。相同 txId 的事务
   * 只会执行一次，重复提交将被忽略。
   *
   * @example 'tx-2024-001'
   */
  txId?: string;

  /**
   * 会话 ID
   *
   * 可选的会话标识符，用于审计和调试。
   *
   * @example 'session-user-123'
   */
  sessionId?: string;
}

/**
 * 判断输入是否符合 NervusDB 打开选项的基本约束
 */
export function isNervusDBOpenOptions(value: unknown): value is NervusDBOpenOptions {
  if (value === null || typeof value !== 'object') {
    return false;
  }

  const options = value as Record<string, unknown>;

  const ensureOptionalBoolean = (key: keyof NervusDBOpenOptions): boolean => {
    if (!(key in options)) return true;
    return typeof options[key] === 'boolean';
  };

  if (!ensureOptionalBoolean('enableLock')) {
    return false;
  }

  if (!ensureOptionalBoolean('registerReader')) {
    return false;
  }

  if ('experimental' in options) {
    const experimental = options.experimental;
    if (experimental !== undefined) {
      if (experimental === null || typeof experimental !== 'object') {
        return false;
      }
      const expRecord = experimental as Record<string, unknown>;
      for (const key of ['cypher', 'gremlin', 'graphql'] as const) {
        if (key in expRecord && typeof expRecord[key] !== 'boolean') {
          return false;
        }
      }
    }
  }

  return true;
}

/**
 * 断言输入符合 NervusDB 打开选项要求
 */
export function assertNervusDBOpenOptions(
  value: unknown,
  message?: string,
): asserts value is NervusDBOpenOptions {
  if (!isNervusDBOpenOptions(value)) {
    throw new TypeError(message ?? 'NervusDB 打开选项格式错误');
  }
}
