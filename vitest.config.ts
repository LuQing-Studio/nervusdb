import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const rootDir = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    testTimeout: 20000,
    // IO 与磁盘操作较多，使用 forks 池进行适度并发，避免内存累积
    pool: 'forks',
    poolOptions: {
      forks: {
        // 允许多进程并行运行，避免单进程内存累积问题
        minForks: 1,
        maxForks: 2, // 减少并发度，降低内存压力
        execArgv: ['--max-old-space-size=8192'] // 增加每个fork进程的内存(应对JIT编译峰值)
      }
    },
    // 禁用文件级并发，确保任何时候只有一个测试在初始化，避免内存峰值
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
