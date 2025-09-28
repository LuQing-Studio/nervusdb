/**
 * SynapseDB 打开选项运行时守卫测试
 */

import { describe, it, expect } from 'vitest';
import { assertSynapseDBOpenOptions, isSynapseDBOpenOptions } from '@/types/openOptions';

describe('SynapseDB 打开选项运行时守卫', () => {
  it('isSynapseDBOpenOptions: 非对象输入返回 false', () => {
    expect(isSynapseDBOpenOptions(null)).toBe(false);
    expect(isSynapseDBOpenOptions(undefined)).toBe(false);
    expect(isSynapseDBOpenOptions(123)).toBe(false);
    expect(isSynapseDBOpenOptions('options')).toBe(false);
  });

  it('isSynapseDBOpenOptions: 检查字段约束', () => {
    expect(isSynapseDBOpenOptions({ pageSize: 0 })).toBe(false);
    expect(isSynapseDBOpenOptions({ pageSize: 1, compression: { codec: 'invalid' } })).toBe(false);
    expect(isSynapseDBOpenOptions({ stagingMode: 'unknown' })).toBe(false);
    expect(isSynapseDBOpenOptions({ enablePersistentTxDedupe: true, maxRememberTxIds: 50 })).toBe(
      false,
    );
  });

  it('isSynapseDBOpenOptions: 合法输入返回 true', () => {
    expect(isSynapseDBOpenOptions({})).toBe(true);
    expect(
      isSynapseDBOpenOptions({
        indexDirectory: '/tmp/index',
        pageSize: 2000,
        rebuildIndexes: false,
        compression: { codec: 'brotli', level: 5 },
        enableLock: true,
        registerReader: true,
        stagingMode: 'default',
        enablePersistentTxDedupe: false,
        maxRememberTxIds: 5000,
      }),
    ).toBe(true);
  });

  it('assertSynapseDBOpenOptions: 非法输入抛出 TypeError，合法输入不抛', () => {
    expect(() => assertSynapseDBOpenOptions({ pageSize: 0 })).toThrowError(TypeError);
    expect(() => assertSynapseDBOpenOptions({ pageSize: 100 })).not.toThrow();
  });
});
