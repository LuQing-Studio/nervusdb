# 使用示例 · 目录总览

本目录提供基于 SynapseDB 的“可直接运行”的使用示例，覆盖 CLI 快速上手、项目接入（本地 tgz / npm link）、查询与联想、事务与幂等、维护治理、流式查询、快照一致性、可视化导出与自动化脚本等场景。当前定位为本地验证（尚未发布到 npm/私有库），后续可演进为类似 Supabase 那样的服务化形态。

- 00-全局 CLI 快速开始：docs/使用示例/00-全局CLI-快速开始.md
- 01-项目接入（本地 tgz 安装）：docs/使用示例/01-项目接入-本地tgz安装.md
- 02-项目接入（npm link 开发联调）：docs/使用示例/02-项目接入-npm-link.md
- 03-查询与联想（基础/链式）：docs/使用示例/03-查询与联想-示例.md
- 04-事务与幂等（批次/耐久）：docs/使用示例/04-事务与幂等-示例.md
- 05-维护治理（compact/gc/repair）：docs/使用示例/05-维护治理-示例.md
- 06-流式查询与大结果：docs/使用示例/06-流式查询与大结果-示例.md
- 07-快照一致性与并发：docs/使用示例/07-快照一致性与并发-示例.md
- 08-图谱导出与可视化：docs/使用示例/08-图谱导出与可视化-示例.md
- 09-嵌入式脚本与自动化：docs/使用示例/09-嵌入式脚本与自动化-示例.md
- 10-消费者项目模板：docs/使用示例/10-消费者项目模板.md
- 99-常见问题与排错：docs/使用示例/99-常见问题与排错.md

前置要求

- Node.js 18+（推荐 20+）
- ESM 环境（package.json: { "type": "module" }）或使用 tsx/ts-node 的 ESM 运行器
- 已全局安装 CLI：SynapseDB CLI

用法:
synapsedb <command> [...args]

命令:
check <db> [--summary|--strict]
repair <db> [--fast]
compact [...args]
stats <db> [--txids[=N]] [--txids-window=MIN]
txids <db> [--list[=N]] [--since=MIN] [--session=ID] [--max=N] [--clear]
dump <db> [...args]
bench <db> [count] [mode]
auto-compact [...args]
gc <db> [...args]
hot <db> [...args]
repair-page <db> <order> <primary>
