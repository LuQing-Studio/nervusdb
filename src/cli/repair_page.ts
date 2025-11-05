#!/usr/bin/env node
import { readPagedManifest } from '../core/storage/pagedIndex.js';
import { repairCorruptedPagesFast } from '../maintenance/repair.js';

async function main() {
  const argv = process.argv.slice(2);
  const positional = argv.filter((arg) => !arg.startsWith('--'));
  const flags = new Set(argv.filter((arg) => arg.startsWith('--')));

  const [dbPath, order, primaryStr] = positional;
  if (!dbPath || !order || !primaryStr) {
    console.log(
      '用法: pnpm db:repair-page <db> <order:SPO|SOP|POS|PSO|OSP|OPS> <primary:number> [--force]',
    );
    console.log('默认 dry-run：仅输出提示信息。使用 --force 才会真实修复。');
    process.exit(1);
  }
  const primary = Number(primaryStr);
  if (!Number.isFinite(primary)) {
    console.error('primary 必须为数字');
    process.exit(1);
  }
  // 将 manifest 标记该页为损坏（注入），然后调用快速修复逻辑
  const indexDir = `${dbPath}.pages`;
  const manifest = await readPagedManifest(indexDir);
  if (!manifest) {
    console.error('缺少 manifest');
    process.exit(2);
  }
  manifest.orphans = manifest.orphans ?? [];

  if (!flags.has('--force')) {
    console.log('⚠️  当前为 dry-run 模式。未执行任何写入。');
    console.log(`   目标数据库: ${dbPath}`);
    console.log(`   目标顺序:   ${order}`);
    console.log(`   目标 primary: ${primary}`);
    console.log('若确认需要修复，请加入 --force 参数。');
    return;
  }

  const res = await repairCorruptedPagesFast(dbPath);
  if (res.repaired.length === 0) {
    console.log('未发现可修复的页；若要强制修复，可先运行 --strict 检查定位');
    return;
  }

  console.log(JSON.stringify(res, null, 2));
}

// eslint-disable-next-line @typescript-eslint/no-floating-promises
main();
