import { describe, expect, it, afterEach } from 'vitest';

import { __setNativeCoreForTesting } from '../../../src/native/core.js';
import { PersistentStore } from '../../../src/core/storage/persistentStore.js';

describe('PersistentStore native bridge', () => {
  afterEach(async () => {
    __setNativeCoreForTesting(undefined);
  });

  it('invokes native binding when available', async () => {
    const calls: Array<{ type: string; payload?: unknown }> = [];

    const mockHandle = {
      addFact(subject: string, predicate: string, object: string) {
        calls.push({ type: 'add', payload: { subject, predicate, object } });
        return { subject_id: 1, predicate_id: 2, object_id: 3 };
      },
      close() {
        calls.push({ type: 'close' });
      },
    };

    __setNativeCoreForTesting({
      open: ({ dataPath }) => {
        calls.push({ type: 'open', payload: dataPath });
        return mockHandle;
      },
    });

    const store = await PersistentStore.open(':memory:');
    store.addFact({ subject: 'alice', predicate: 'knows', object: 'bob' });
    await store.close();

    expect(calls.find((c) => c.type === 'open')).toBeTruthy();
    expect(calls.find((c) => c.type === 'add')).toMatchObject({
      payload: { subject: 'alice', predicate: 'knows', object: 'bob' },
    });
    expect(calls.some((c) => c.type === 'close')).toBe(true);
  });
});
