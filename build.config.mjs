/**
 * NervusDB Build Configuration
 * 使用 esbuild 打包和混淆代码
 */

import { build } from 'esbuild';
import { glob } from 'glob';
import fs from 'fs';
import path from 'path';

const outdir = 'dist';

async function buildBundle() {
  console.log('🔨 Building NervusDB...');

  // 清理旧的 dist
  if (fs.existsSync(outdir)) {
    fs.rmSync(outdir, { recursive: true });
  }

  // 1. 构建主库 (ESM)
  await build({
    entryPoints: ['src/index.ts'],
    bundle: true,
    platform: 'node',
    target: 'node18',
    format: 'esm',
    outfile: `${outdir}/index.mjs`,
    minify: true, // 混淆和压缩
    sourcemap: false, // 不生成 source map
    treeShaking: true, // 移除未使用代码
    external: [
      // 不打包的外部依赖（如果有）
    ],
    banner: {
      js: '// NervusDB - Neural Knowledge Graph Database\n// (c) 2025. All rights reserved.\n',
    },
  });

  // 2. 构建 CLI (单独打包，包含所有依赖)
  await build({
    entryPoints: ['src/cli/nervusdb.ts'],
    bundle: true,
    platform: 'node',
    target: 'node18',
    format: 'esm',
    outfile: `${outdir}/cli/nervusdb.js`,
    minify: true,
    sourcemap: false,
    treeShaking: true,
    banner: {
      js: '#!/usr/bin/env node\n// NervusDB CLI\n// (c) 2025. All rights reserved.\n',
    },
  });

  // 3. 生成类型定义（使用 tsc）
  console.log('📝 Generating type definitions...');
  const { execSync } = await import('child_process');
  execSync('tsc --project tsconfig.build.json --emitDeclarationOnly', {
    stdio: 'inherit',
  });

  // 4. 设置 CLI 可执行权限
  fs.chmodSync(`${outdir}/cli/nervusdb.js`, 0o755);

  console.log('✅ Build complete!');
  console.log(`📦 Output: ${outdir}/`);
}

buildBundle().catch((err) => {
  console.error('❌ Build failed:', err);
  process.exit(1);
});
