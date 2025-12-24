# T36: 发布准备 v1.0.3（Rust + npm）

## 1. Context

当前仓库版本号已不一致：

- crates.io：`nervusdb-core = 1.0.1`
- npm：`@nervusdb/core = 1.0.2`
- 仓库内：Rust/Python/native addon 多处仍为 `1.0.1`

同时，`docs/` 根目录存在多份散落文档，不符合项目的文档分层约束（根目录应仅保留 `docs/task_progress.md`，其余必须归档到子目录）。

本任务目标是把发布前“版本、文档、验证”一次性收口，避免发布时临时补丁导致不可逆的注册表污染。

## 2. Goals

- 版本号统一到 `1.0.3`（Rust + Node 为发布目标；Python 仅同步版本号，不执行发布）。
- `docs/` 根目录只保留 `docs/task_progress.md`，其余文档移动到合理子目录，并修复引用链接。
- 发布前验证：
  - Rust：`cargo test` + `cargo publish --dry-run`
  - Node：`pnpm -C bindings/node ci:prepush` + `npm publish --dry-run`

## 3. Non-Goals

- 不在仓库里写入任何 token/凭证，也不在聊天里粘贴任何 token。
- 不执行真实发布（`cargo publish` / `npm publish`），只做 dry-run 和可发布性验证。

## 4. Plan

1. 文档整理：
   - `docs/release/`：发布相关（`publishing.md`）
   - `docs/reference/`：参考文档（`cypher_support.md`、`project-structure.md`）
   - `docs/perf/`：性能报告（`PERFORMANCE_ANALYSIS.md`）
2. 版本号统一到 `1.0.3`：
   - Rust：`nervusdb-core`、`nervusdb-temporal`
   - Node：`bindings/node/package.json`、`bindings/node/native/nervusdb-node/Cargo.toml`
   - Python：`bindings/python/nervusdb-py/pyproject.toml`、`bindings/python/nervusdb-py/Cargo.toml`
3. `CHANGELOG.md` 增加 `1.0.3` 条目。
4. 本地构建/测试验证（见 Goals）。

## 5. Risks

- docs 移动会打断旧链接：必须全局搜索并修复引用。
- crates.io/npm 不允许覆盖同版本：版本号必须前进到未占用的新版本（本次为 `1.0.3`）。

