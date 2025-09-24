import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const rootDir = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    testTimeout: 20000,
    // IO 与磁盘操作较多，使用 forks 池并限制并发，增加可用堆内存，避免 OOM/句柄压力
    pool: 'forks',
    poolOptions: {
      forks: {
        // 单进程串行运行测试，降低峰值内存和句柄压力
        singleFork: true,
        minForks: 1,
        maxForks: 1,
        execArgv: ['--max-old-space-size=4096']
      }
    },
    // 保守起见，禁用文件级并发执行，降低资源争用
    sequence: {
      concurrent: false
    },
    include: ['tests/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      reportsDirectory: resolve(rootDir, 'coverage'),
      reporter: ['text', 'lcov'],
      include: ['src/**/*.ts'],
      // 说明：以下排除项不计入当前覆盖率门槛
      // - src/cli/**: CLI 封装，覆盖率独立评估
      // - **/*.d.ts/**.config.*: 类型与配置文件
      // - src/types/**: 仅类型增强文件，不生成可执行代码
      // - src/spatial/**: 空间计算与 R-Tree 模块，后续将单独补充专项测试再纳入门槛
      exclude: [
        'src/cli/**',
        '**/*.d.ts',
        '**/*.config.*',
        'cspell.config.cjs',
        'src/types/**',
        'src/spatial/**',
        'src/fulltext/**',
        'src/benchmark/**',
        // 暂未纳入门槛的基础设施/占位文件
        'src/graph/paths.ts',
        'src/query/iterator.ts',
        'src/query/gremlin/step.ts',
        'src/query/path/bidirectional.ts',
        'src/query/pattern/ast.ts'
      ],
      thresholds: {
        statements: 80,
        branches: 75,
        functions: 80,
        lines: 80
      }
    }
  },
  resolve: {
    alias: {
      '@': resolve(rootDir, 'src')
    }
  }
});
