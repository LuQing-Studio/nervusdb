import { promises as fs } from 'node:fs';
import { join } from 'node:path';

import {
  readPagedManifest,
  writePagedManifest,
  pageFileName,
  type PagedIndexManifest,
} from '../core/storage/pagedIndex.js';
import { getActiveReaders } from '../core/storage/readerRegistry.js';
import { triggerCrash } from '../utils/fault.js';

export interface GCStats {
  orders: Array<{ order: string; bytesBefore: number; bytesAfter: number; pages: number }>;
  bytesBefore: number;
  bytesAfter: number;
  skipped?: boolean;
  reason?: string;
  readers?: number;
  dryRun?: boolean;
}

export async function garbageCollectPages(
  dbPath: string,
  options?: { respectReaders?: boolean; dryRun?: boolean },
): Promise<GCStats> {
  const indexDir = `${dbPath}.pages`;
  const manifest = await readPagedManifest(indexDir);
  if (!manifest) throw new Error('缺少 manifest，无法进行 GC');

  const dryRun = options?.dryRun ?? false;

  if (options?.respectReaders) {
    const readers = await getActiveReaders(indexDir);
    if (readers.length > 0) {
      return {
        orders: [],
        bytesBefore: 0,
        bytesAfter: 0,
        skipped: true,
        reason: 'active_readers',
        readers: readers.length,
        dryRun,
      };
    }
  }

  let bytesBefore = 0;
  let bytesAfter = 0;
  const orderStats: GCStats['orders'] = [];

  for (const lookup of manifest.lookups) {
    const file = join(indexDir, pageFileName(lookup.order));
    let st;
    try {
      st = await fs.stat(file);
    } catch {
      orderStats.push({
        order: lookup.order,
        bytesBefore: 0,
        bytesAfter: 0,
        pages: lookup.pages.length,
      });
      continue;
    }
    const predictedSize = lookup.pages.reduce((sum, page) => sum + page.length, 0);
    bytesBefore += st.size;

    if (dryRun) {
      bytesAfter += predictedSize;
      orderStats.push({
        order: lookup.order,
        bytesBefore: st.size,
        bytesAfter: predictedSize,
        pages: lookup.pages.length,
      });
      continue;
    }

    const tmp = `${file}.gc.tmp`;
    try {
      await fs.unlink(tmp);
    } catch {}

    let src: fs.FileHandle | null = null;
    let dst: fs.FileHandle | null = null;
    let offset = 0;
    const newPages: typeof lookup.pages = [];
    try {
      src = await fs.open(file, 'r');
      dst = await fs.open(tmp, 'w');

      for (const page of lookup.pages) {
        const buf = Buffer.allocUnsafe(page.length);
        await src.read(buf, 0, page.length, page.offset);
        await dst.write(buf, 0, buf.length, offset);
        newPages.push({
          primaryValue: page.primaryValue,
          offset,
          length: page.length,
          rawLength: page.rawLength,
          crc32: page.crc32,
        });
        offset += page.length;
      }
      await dst.sync();
    } finally {
      if (src) await src.close();
      if (dst) await dst.close();
    }
    triggerCrash('gc.beforeRename');
    await fs.rename(tmp, file);
    triggerCrash('gc.afterRename');
    // 更新该顺序的 pages 映射（offset 变化）
    lookup.pages = newPages;

    const stAfter = await fs.stat(file);
    bytesAfter += stAfter.size;
    orderStats.push({
      order: lookup.order,
      bytesBefore: st.size,
      bytesAfter: stAfter.size,
      pages: newPages.length,
    });
  }

  const newManifest: PagedIndexManifest = {
    ...manifest,
    epoch: (manifest.epoch ?? 0) + 1,
    orphans: [],
  };
  triggerCrash('gc.beforeManifestWrite');
  await writePagedManifest(indexDir, newManifest);

  return { orders: orderStats, bytesBefore, bytesAfter, dryRun };
}
