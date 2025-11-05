import { promises as fsp } from 'node:fs';
import { join } from 'node:path';

import { readStorageFile } from '../fileHeader.js';
import { EncodedTriple, TripleStore } from '../tripleStore.js';
import { TripleIndexes, type IndexOrder } from '../tripleIndexes.js';
import {
  DEFAULT_PAGE_SIZE,
  PagedIndexReader,
  PagedIndexWriter,
  pageFileName,
  readPagedManifest,
  writePagedManifest,
  type PageMeta,
  type PagedIndexManifest,
} from '../pagedIndex.js';
import { decodeTripleKey, encodeTripleKey, primarySelector } from '../helpers/tripleOrdering.js';

export interface PagedIndexCoordinatorOptions {
  indexDirectory: string;
}

export interface RebuildOptions {
  pageSize?: number;
  compression?: { codec: 'none' | 'brotli'; level?: number };
}

export interface AppendOptions {
  staged: TripleIndexes;
  tombstones: Set<string>;
  pageSize?: number;
  includeTombstones?: boolean;
}

const ORDERS: IndexOrder[] = ['SPO', 'SOP', 'POS', 'PSO', 'OSP', 'OPS'];

export class PagedIndexCoordinator {
  private readonly readers = new Map<IndexOrder, PagedIndexReader>();
  private manifest: PagedIndexManifest | null = null;
  private currentEpoch = 0;

  constructor(private readonly options: PagedIndexCoordinatorOptions) {}

  getReader(order: IndexOrder): PagedIndexReader | undefined {
    return this.readers.get(order);
  }

  getCurrentEpoch(): number {
    return this.currentEpoch;
  }

  getManifest(): PagedIndexManifest | null {
    return this.manifest;
  }

  async loadManifest(tombstones: Set<string>): Promise<PagedIndexManifest | null> {
    const manifest = await readPagedManifest(this.options.indexDirectory);
    if (manifest) {
      this.applyManifest(manifest, tombstones);
    }
    return manifest;
  }

  async rebuildFromStorage(
    dbPath: string,
    tombstones: Set<string>,
    options: RebuildOptions = {},
  ): Promise<PagedIndexManifest> {
    await fsp.mkdir(this.options.indexDirectory, { recursive: true });
    const pageSize = options.pageSize ?? DEFAULT_PAGE_SIZE;
    const compression = options.compression ?? { codec: 'none' }; // 与旧实现一致

    const lookups: Array<{ order: IndexOrder; pages: PageMeta[] }> = [];
    const sections = await readStorageFile(dbPath);
    const historicalTriples = TripleStore.deserialize(sections.triples);
    const triples = historicalTriples.list();

    for (const order of ORDERS) {
      const filePath = join(this.options.indexDirectory, pageFileName(order));
      try {
        await fsp.unlink(filePath);
      } catch {
        // 无文件可删时忽略
      }
      const writer = new PagedIndexWriter(filePath, {
        directory: this.options.indexDirectory,
        pageSize,
        compression,
      });
      const selector = primarySelector(order);
      for (const triple of triples) {
        writer.push(triple, selector(triple));
      }
      const pages = await writer.finalize();
      this.readers.set(
        order,
        new PagedIndexReader(
          { directory: this.options.indexDirectory, compression },
          { order, pages },
        ),
      );
      lookups.push({ order, pages });
    }

    const manifest: PagedIndexManifest = {
      version: 1,
      pageSize,
      createdAt: Date.now(),
      compression,
      lookups,
      epoch: (this.manifest?.epoch ?? 0) + 1,
    };

    await writePagedManifest(this.options.indexDirectory, manifest);
    this.applyManifest(manifest, tombstones);
    return manifest;
  }

  async appendFromStaging(options: AppendOptions): Promise<PagedIndexManifest> {
    await fsp.mkdir(this.options.indexDirectory, { recursive: true });
    const manifest = (await readPagedManifest(this.options.indexDirectory)) ?? {
      version: 1,
      pageSize: options.pageSize ?? DEFAULT_PAGE_SIZE,
      createdAt: Date.now(),
      compression: { codec: 'none' as const },
      lookups: [],
    };

    const effectivePageSize =
      options.pageSize && options.pageSize !== DEFAULT_PAGE_SIZE
        ? options.pageSize
        : (manifest.pageSize ?? DEFAULT_PAGE_SIZE);

    const lookupMap = new Map<IndexOrder, { order: IndexOrder; pages: PageMeta[] }>(
      manifest.lookups.map((lookup) => [
        lookup.order,
        { order: lookup.order, pages: [...lookup.pages] },
      ]),
    );

    const { lsmTriples, segmentsToRemove } = await this.readLsmSegments();

    for (const order of ORDERS) {
      const stagedTriples = options.staged.get(order);
      const extraTriples = lsmTriples;
      if (stagedTriples.length === 0 && extraTriples.length === 0) {
        continue;
      }
      const filePath = join(this.options.indexDirectory, pageFileName(order));
      const writer = new PagedIndexWriter(filePath, {
        directory: this.options.indexDirectory,
        pageSize: effectivePageSize,
        compression: manifest.compression,
      });
      const selector = primarySelector(order);
      for (const triple of stagedTriples) {
        writer.push(triple, selector(triple));
      }
      for (const triple of extraTriples) {
        writer.push(triple, selector(triple));
      }
      const pages = await writer.finalize();
      const existed = lookupMap.get(order) ?? { order, pages: [] };
      existed.pages.push(...pages);
      lookupMap.set(order, existed);
    }

    const newManifest: PagedIndexManifest = {
      version: 1,
      pageSize: effectivePageSize,
      createdAt: Date.now(),
      compression: manifest.compression,
      lookups: [...lookupMap.values()],
      epoch: (manifest.epoch ?? 0) + 1,
    };

    if (options.includeTombstones) {
      newManifest.tombstones = [...options.tombstones]
        .map((key) => decodeTripleKey(key))
        .map(
          ({ subjectId, predicateId, objectId }) =>
            [subjectId, predicateId, objectId] as [number, number, number],
        );
    }

    await writePagedManifest(this.options.indexDirectory, newManifest);
    this.applyManifest(newManifest, options.tombstones);

    if (segmentsToRemove.length > 0) {
      await this.cleanupLsmSegments(segmentsToRemove);
    }

    return newManifest;
  }

  private applyManifest(manifest: PagedIndexManifest, tombstones: Set<string>): void {
    this.manifest = manifest;
    this.currentEpoch = manifest.epoch ?? this.currentEpoch;
    this.readers.clear();
    for (const lookup of manifest.lookups) {
      this.readers.set(
        lookup.order,
        new PagedIndexReader(
          { directory: this.options.indexDirectory, compression: manifest.compression },
          lookup,
        ),
      );
    }
    if (manifest.tombstones?.length) {
      tombstones.clear();
      for (const [subjectId, predicateId, objectId] of manifest.tombstones) {
        tombstones.add(encodeTripleKey({ subjectId, predicateId, objectId }));
      }
    }
  }

  private async readLsmSegments(): Promise<{
    lsmTriples: EncodedTriple[];
    segmentsToRemove: string[];
  }> {
    const triples: EncodedTriple[] = [];
    const segments: string[] = [];
    try {
      const manPath = join(this.options.indexDirectory, 'lsm-manifest.json');
      const buf = await fsp.readFile(manPath);
      const manifest = JSON.parse(buf.toString('utf8')) as {
        segments?: Array<{ file: string }>;
      };
      for (const seg of manifest.segments ?? []) {
        const filePath = join(this.options.indexDirectory, seg.file);
        try {
          const data = await fsp.readFile(filePath);
          const count = Math.floor(data.length / 12);
          for (let i = 0; i < count; i += 1) {
            const offset = i * 12;
            triples.push({
              subjectId: data.readUInt32LE(offset),
              predicateId: data.readUInt32LE(offset + 4),
              objectId: data.readUInt32LE(offset + 8),
            });
          }
          segments.push(filePath);
        } catch {
          // 单个段失败时忽略
        }
      }
    } catch {
      // 无清单时忽略
    }
    return { lsmTriples: triples, segmentsToRemove: segments };
  }

  private async cleanupLsmSegments(segments: string[]): Promise<void> {
    const manifestPath = join(this.options.indexDirectory, 'lsm-manifest.json');
    for (const file of segments) {
      try {
        await fsp.unlink(file);
      } catch {
        // ignore
      }
    }
    try {
      await fsp.writeFile(
        manifestPath,
        JSON.stringify({ version: 1, segments: [] }, null, 2),
        'utf8',
      );
      try {
        const dirHandle = await fsp.open(this.options.indexDirectory, 'r');
        try {
          await dirHandle.sync();
        } finally {
          await dirHandle.close();
        }
      } catch {
        // ignore directory sync failure
      }
    } catch {
      // ignore
    }
  }
}
