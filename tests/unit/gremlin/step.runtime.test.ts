/**
 * Gremlin Step 运行时工具测试
 *
 * 覆盖 src/query/gremlin/step.ts 中的 isStep/assertStep 分支，
 * 避免该文件覆盖率为 0。
 */

import { describe, it, expect } from 'vitest';
import { assertStep, isStep } from '@/query/gremlin/step';

describe('Gremlin Step 运行时工具', () => {
  it('isStep: 非对象/空值返回 false', () => {
    expect(isStep(null)).toBe(false);
    expect(isStep(undefined)).toBe(false);
    expect(isStep(123)).toBe(false);
    expect(isStep('step')).toBe(false);
    expect(isStep([])).toBe(false);
  });

  it('isStep: 缺少字段返回 false', () => {
    expect(isStep({})).toBe(false);
    expect(isStep({ type: 'V' })).toBe(false);
    expect(isStep({ id: 'x' })).toBe(false);
  });

  it('isStep: 含有 type/id 字段返回 true', () => {
    expect(isStep({ type: 'V', id: '1' })).toBe(true);
    expect(isStep({ type: 'out', id: 'a', extra: 1 })).toBe(true);
  });

  it('assertStep: 非法对象抛出 TypeError；合法对象不抛', () => {
    expect(() => assertStep({})).toThrowError(TypeError);
    expect(() => assertStep({ type: 'E', id: '2' })).not.toThrow();
  });
});
