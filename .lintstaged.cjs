/** lint-staged v15+ 显式配置（CommonJS） */
module.exports = {
  'src/**/*.{ts,tsx}': ['pnpm exec eslint --fix --max-warnings=0'],
  'tests/**/*.ts': ['pnpm exec eslint --fix --max-warnings=0']
};
