#!/usr/bin/env node
import { getActiveReaders } from '../storage/readerRegistry.js';

async function main() {
  const [dbPath, ...args] = process.argv.slice(2);
  if (!dbPath || dbPath === '--help' || dbPath === '-h') {
    console.log('用法: pnpm db:readers <db> [选项]');
    console.log('选项:');
    console.log('  --json              输出 JSON 格式');
    console.log('  --watch             持续监控模式（每5秒刷新）');
    console.log('  --details           显示详细的读者信息');
    console.log('');
    console.log('说明:');
    console.log('  显示当前数据库的活跃读者注册信息，用于诊断并发访问情况。');
    console.log('  读者注册表记录了正在访问数据库的进程信息，有助于理解');
    console.log('  为什么某些维护操作（如压缩、垃圾回收）被跳过。');
    process.exit(1);
  }

  const opts: Record<string, boolean> = {};
  for (const a of args) {
    if (a.startsWith('--')) {
      opts[a.substring(2)] = true;
    }
  }

  const outputJson = opts['json'];
  const watchMode = opts['watch'];
  const showDetails = opts['details'];

  async function displayReaders() {
    try {
      const readers = await getActiveReaders(`${dbPath}.pages`);
      const now = Date.now();

      if (outputJson) {
        const result = {
          timestamp: new Date().toISOString(),
          totalReaders: readers.length,
          readers: readers.map((reader) => ({
            pid: reader.pid,
            epoch: reader.epoch,
            registeredAt: new Date(reader.ts).toISOString(),
            ageMs: now - reader.ts,
            ageSec: Math.round((now - reader.ts) / 1000),
            // ReaderInfo 当前不包含 sessionId 字段
            // sessionId: reader.sessionId || null,
          })),
        };
        console.log(JSON.stringify(result, null, 2));
        return;
      }

      // 非 JSON 模式的表格输出
      console.log(`📊 Active Database Readers - ${new Date().toLocaleString()}`);
      console.log(`Database: ${dbPath}`);
      console.log(`Total active readers: ${readers.length}`);

      if (readers.length === 0) {
        console.log('✅ No active readers - maintenance operations can proceed safely');
        return;
      }

      console.log('');

      // 表格头部
      const headers = ['PID', 'Epoch', 'Age', 'Registered At'];
      // Note: SessionId 不在 ReaderInfo 中，暂时不显示

      // 计算列宽
      const colWidths = [
        Math.max(8, ...readers.map((r) => String(r.pid).length)),
        Math.max(6, ...readers.map((r) => String(r.epoch).length)),
        Math.max(12, ...readers.map((r) => formatAge(now - r.ts).length)),
        Math.max(19, ...readers.map((r) => new Date(r.ts).toLocaleString().length)),
      ];

      // 打印表格头部
      const headerRow = headers.map((h, i) => h.padEnd(colWidths[i])).join(' | ');
      console.log(headerRow);
      console.log('-'.repeat(headerRow.length));

      // 打印数据行
      for (const reader of readers) {
        const row = [
          String(reader.pid).padEnd(colWidths[0]),
          String(reader.epoch).padEnd(colWidths[1]),
          formatAge(now - reader.ts).padEnd(colWidths[2]),
          new Date(reader.ts).toLocaleString().padEnd(colWidths[3]),
        ];
        console.log(row.join(' | '));
      }

      // 统计信息
      if (showDetails && readers.length > 0) {
        console.log('');
        console.log('📈 Summary:');
        const epochs = readers.map((r) => r.epoch);
        const minEpoch = Math.min(...epochs);
        const maxEpoch = Math.max(...epochs);
        const avgAge = Math.round(
          readers.reduce((sum, r) => sum + (now - r.ts), 0) / readers.length / 1000,
        );

        console.log(`   Epoch range: ${minEpoch} - ${maxEpoch}`);
        console.log(`   Average reader age: ${avgAge}s`);

        // 按 epoch 分组统计
        const epochGroups = new Map<number, number>();
        for (const reader of readers) {
          epochGroups.set(reader.epoch, (epochGroups.get(reader.epoch) || 0) + 1);
        }
        console.log(
          `   Readers by epoch: ${Array.from(epochGroups.entries())
            .map(([e, c]) => `${e}(${c})`)
            .join(', ')}`,
        );
      }
    } catch (error) {
      if (outputJson) {
        console.log(
          JSON.stringify(
            {
              error: `Failed to read reader registry: ${String(error)}`,
              timestamp: new Date().toISOString(),
              totalReaders: 0,
              readers: [],
            },
            null,
            2,
          ),
        );
      } else {
        console.error(`❌ Failed to read reader registry: ${String(error)}`);
        console.log('💡 This could mean:');
        console.log('   • Database has no paged indexes yet');
        console.log('   • Reader registry is not initialized');
        console.log('   • Permission issues accessing .pages directory');
      }
    }
  }

  if (watchMode) {
    console.log('🔄 Watch mode enabled - press Ctrl+C to exit');
    console.log('');

    let first = true;
    while (true) {
      if (!first) {
        // 清屏并回到顶部
        process.stdout.write('\x1B[2J\x1B[0f');
      }
      first = false;

      await displayReaders();

      if (!outputJson) {
        console.log('\n⏰ Refreshing in 5 seconds...');
      }

      await new Promise((resolve) => setTimeout(resolve, 5000));
    }
  } else {
    await displayReaders();
  }
}

function formatAge(ageMs: number): string {
  const seconds = Math.floor(ageMs / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) {
    return `${days}d ${hours % 24}h`;
  } else if (hours > 0) {
    return `${hours}h ${minutes % 60}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  } else {
    return `${seconds}s`;
  }
}

// eslint-disable-next-line @typescript-eslint/no-floating-promises
main();
