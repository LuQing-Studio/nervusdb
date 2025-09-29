#!/usr/bin/env node
import { garbageCollectPages } from '../maintenance/gc.js';

async function main() {
  const [dbPath, ...args] = process.argv.slice(2);
  if (!dbPath) {
    console.log('用法: pnpm db:gc <db> [--no-respect-readers] [--dry-run] [--force]');
    console.log('默认 dry-run；使用 --force 才会真实回收页面。');
    process.exit(1);
  }
  const flags = new Set(args);
  const respect = !flags.has('--no-respect-readers');
  const dryRun = flags.has('--force') ? false : true;
  const stats = await garbageCollectPages(dbPath, { respectReaders: respect, dryRun });
  console.log(JSON.stringify(stats, null, 2));
}

// eslint-disable-next-line @typescript-eslint/no-floating-promises
main();
