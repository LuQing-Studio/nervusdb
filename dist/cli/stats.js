#!/usr/bin/env node
import { promises as fs } from 'node:fs';
import { join } from 'node:path';
import { readStorageFile } from '../storage/fileHeader.js';
import { readPagedManifest } from '../storage/pagedIndex.js';
import { PropertyIndexManager } from '../storage/propertyIndex.js';
import { readHotness } from '../storage/hotness.js';
import { getActiveReaders } from '../storage/readerRegistry.js';
async function stats(dbPath, opts) {
    const sections = await readStorageFile(dbPath);
    const dictCount = sections.dictionary.length >= 4 ? sections.dictionary.readUInt32LE(0) : 0;
    const tripleCount = sections.triples.length >= 4 ? sections.triples.readUInt32LE(0) : 0;
    const indexDir = `${dbPath}.pages`;
    const manifest = await readPagedManifest(indexDir);
    const lookups = manifest?.lookups ?? [];
    const epoch = manifest?.epoch ?? 0;
    const tombstones = manifest?.tombstones?.length ?? 0;
    let pageFiles = 0;
    let pages = 0;
    const orders = {};
    for (const l of lookups) {
        pageFiles += 1;
        pages += l.pages.length;
        const cnt = new Map();
        for (const p of l.pages)
            cnt.set(p.primaryValue, (cnt.get(p.primaryValue) ?? 0) + 1);
        const multi = [...cnt.values()].filter((c) => c > 1).length;
        orders[l.order] = { pages: l.pages.length, primaries: cnt.size, multiPagePrimaries: multi };
    }
    let walSize = 0;
    try {
        const st = await fs.stat(`${dbPath}.wal`);
        walSize = st.size;
    }
    catch { }
    // txId 注册表（若存在）
    let txIds = 0;
    let txIdItems;
    let txIdsWindow = 0;
    let txIdsBySession;
    let lsmSegments = 0;
    let lsmTriples = 0;
    try {
        const { readTxIdRegistry } = await import('../storage/txidRegistry.js');
        const reg = await readTxIdRegistry(`${dbPath}.pages`);
        txIds = reg.txIds.length;
        if (opts.listTxIds && opts.listTxIds > 0) {
            txIdItems = [...reg.txIds].sort((a, b) => b.ts - a.ts).slice(0, opts.listTxIds);
        }
        if (opts.txIdsWindowMin && opts.txIdsWindowMin > 0) {
            const since = Date.now() - opts.txIdsWindowMin * 60_000;
            const items = reg.txIds.filter((x) => x.ts >= since);
            txIdsWindow = items.length;
            const g = {};
            for (const it of items) {
                const key = it.sessionId ?? 'unknown';
                g[key] = (g[key] ?? 0) + 1;
            }
            txIdsBySession = g;
        }
    }
    catch { }
    // LSM-Lite 段清单（实验性）
    try {
        const man = await fs.readFile(`${dbPath}.pages/lsm-manifest.json`);
        const m = JSON.parse(man.toString('utf8'));
        lsmSegments = m.segments?.length ?? 0;
        lsmTriples = (m.segments ?? []).reduce((a, s) => a + (s.count ?? 0), 0);
    }
    catch { }
    // 属性索引统计
    let propertyStats = undefined;
    if (opts.includeProperty) {
        try {
            const propertyManager = new PropertyIndexManager(`${dbPath}.pages`);
            await propertyManager.initialize();
            const memIndex = propertyManager.getMemoryIndex();
            const stats = memIndex.getStats();
            propertyStats = {
                nodePropertyCount: stats.nodePropertyCount,
                edgePropertyCount: stats.edgePropertyCount,
                totalNodeEntries: stats.totalNodeEntries,
                totalEdgeEntries: stats.totalEdgeEntries,
            };
            if (opts.verbose) {
                propertyStats.nodePropertyNames = memIndex.getNodePropertyNames();
                propertyStats.edgePropertyNames = memIndex.getEdgePropertyNames();
            }
        }
        catch (error) {
            propertyStats = { error: `Failed to read property index: ${String(error)}` };
        }
    }
    // 热度统计
    let hotnessStats = undefined;
    if (opts.includeHotness) {
        try {
            const hotness = await readHotness(`${dbPath}.pages`);
            const totalAccesses = Object.values(hotness.counts).reduce((sum, orderCounts) => {
                return sum + Object.values(orderCounts).reduce((a, b) => a + b, 0);
            }, 0);
            hotnessStats = {
                version: hotness.version,
                updatedAt: new Date(hotness.updatedAt).toISOString(),
                totalAccesses,
                accessesByOrder: Object.entries(hotness.counts).map(([order, counts]) => ({
                    order,
                    accessCount: Object.values(counts).reduce((a, b) => a + b, 0),
                    uniquePrimaries: Object.keys(counts).length,
                })),
            };
            if (opts.verbose) {
                // 显示热门主键（前10个）
                hotnessStats.topHotPrimaries = Object.entries(hotness.counts).map(([order, counts]) => {
                    const sorted = Object.entries(counts)
                        .sort(([, a], [, b]) => b - a)
                        .slice(0, 10);
                    return {
                        order,
                        hotPrimaries: sorted.map(([primary, count]) => ({
                            primary: Number(primary),
                            accessCount: count,
                        })),
                    };
                });
            }
        }
        catch (error) {
            hotnessStats = { error: `Failed to read hotness data: ${String(error)}` };
        }
    }
    // 读者注册信息
    let readersStats = undefined;
    if (opts.includeReaders) {
        try {
            const readers = await getActiveReaders(`${dbPath}.pages`);
            readersStats = {
                totalReaders: readers.length,
                readers: readers.map((reader) => ({
                    pid: reader.pid,
                    epoch: reader.epoch,
                    registeredAt: new Date(reader.ts).toISOString(),
                    ageMs: Date.now() - reader.ts,
                })),
            };
            if (readers.length > 0) {
                const epochs = readers.map((r) => r.epoch);
                readersStats.epochRange = {
                    min: Math.min(...epochs),
                    max: Math.max(...epochs),
                    current: epoch,
                };
            }
        }
        catch (error) {
            readersStats = { error: `Failed to read reader registry: ${String(error)}` };
        }
    }
    // 数据库文件大小统计
    let fileSizeStats = undefined;
    if (opts.verbose) {
        try {
            const mainFileStat = await fs.stat(dbPath);
            fileSizeStats = {
                mainFileBytes: mainFileStat.size,
                walBytes: walSize,
            };
            // 计算分页索引文件大小
            let totalPageBytes = 0;
            for (const lookup of lookups) {
                try {
                    const pageFilePath = join(`${dbPath}.pages`, `${lookup.order}.pages`);
                    const pageFileStat = await fs.stat(pageFilePath);
                    totalPageBytes += pageFileStat.size;
                }
                catch { }
            }
            fileSizeStats.totalPageBytes = totalPageBytes;
            fileSizeStats.totalBytes = mainFileStat.size + walSize + totalPageBytes;
            // 计算压缩率（如果启用了压缩）
            if (manifest && manifest.compression?.codec !== 'none') {
                fileSizeStats.compression = manifest.compression;
            }
        }
        catch (error) {
            fileSizeStats = { error: `Failed to read file sizes: ${String(error)}` };
        }
    }
    const out = {
        dictionaryEntries: dictCount,
        triples: tripleCount,
        epoch,
        pageFiles,
        pages,
        tombstones,
        walBytes: walSize,
        txIds,
        lsmSegments,
        lsmTriples,
        orders,
    };
    // 添加新的统计信息
    if (propertyStats)
        out.propertyIndex = propertyStats;
    if (hotnessStats)
        out.hotness = hotnessStats;
    if (readersStats)
        out.readers = readersStats;
    if (fileSizeStats)
        out.fileSizes = fileSizeStats;
    // 原有的可选统计
    if (txIdItems)
        out.txIdItems = txIdItems;
    if (opts.txIdsWindowMin) {
        out.txIdsWindowMin = opts.txIdsWindowMin;
        out.txIdsWindow = txIdsWindow;
        if (txIdsBySession)
            out.txIdsBySession = txIdsBySession;
    }
    // 添加汇总统计（当启用详细模式时）
    if (opts.verbose) {
        out.summary = {
            totalDataStructures: 1 + pageFiles + (propertyStats ? 1 : 0), // main + pages + property index
            totalEntries: dictCount +
                tripleCount +
                (propertyStats?.totalNodeEntries ?? 0) +
                (propertyStats?.totalEdgeEntries ?? 0),
            indexEfficiency: pages > 0 ? (tripleCount / pages).toFixed(2) : 'N/A',
            compressionEnabled: manifest?.compression?.codec !== 'none',
        };
    }
    console.log(JSON.stringify(out, null, 2));
}
async function main() {
    const args = process.argv.slice(2);
    const dbPath = args[0];
    if (!dbPath) {
        console.log('用法: pnpm db:stats <db> [选项]');
        console.log('选项:');
        console.log('  --verbose               显示详细统计信息');
        console.log('  --property              包含属性索引统计');
        console.log('  --hotness              包含热度统计');
        console.log('  --readers              包含读者注册信息');
        console.log('  --all                  包含所有扩展统计（等同于 --property --hotness --readers --verbose）');
        console.log('  --txids[=N]            显示最近N个事务ID（默认50）');
        console.log('  --txids-window=M       显示M分钟内的事务ID统计');
        process.exit(1);
    }
    const verbose = args.includes('--verbose');
    const includeProperty = args.includes('--property');
    const includeHotness = args.includes('--hotness');
    const includeReaders = args.includes('--readers');
    const all = args.includes('--all');
    const listArg = args.find((a) => a.startsWith('--txids'));
    let listTxIds;
    if (listArg) {
        const parts = listArg.split('=');
        listTxIds = parts.length > 1 ? Number(parts[1]) : 50;
    }
    const winArg = args.find((a) => a.startsWith('--txids-window='));
    const txIdsWindowMin = winArg ? Number(winArg.split('=')[1]) : undefined;
    await stats(dbPath, {
        listTxIds,
        txIdsWindowMin,
        verbose: verbose || all,
        includeProperty: includeProperty || all,
        includeHotness: includeHotness || all,
        includeReaders: includeReaders || all,
    });
}
// eslint-disable-next-line @typescript-eslint/no-floating-promises
main();
//# sourceMappingURL=stats.js.map