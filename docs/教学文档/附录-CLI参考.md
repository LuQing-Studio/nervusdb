# 附录 · CLI 参考

> SynapseDB 提供完整的命令行工具链，覆盖诊断、治理、导出与事务观测。本附录列出主要命令及常用参数。

## 总体说明

- 默认命令：`synapsedb <command> [options]`
- 源码模式：`pnpm db:<command>` 等价
- 所有命令支持绝对/相对路径，建议显式带文件扩展名

## 常用命令速查

| 类别     | 命令                     | 说明                                             |
| -------- | ------------------------ | ------------------------------------------------ |
| 统计     | `stats`                  | 输出三元组数量、页分布、热度、事务 ID 等         |
| 检查     | `check`                  | 校验索引、manifest、WAL，`--strict` 开启深度校验 |
| 修复     | `repair` / `repair-page` | 自动修复页映射或指定主键重建                     |
| 压实     | `compact`                | 手动压实，支持 rewrite/incremental               |
| 自动治理 | `auto-compact`           | 基于热度与墓碑阈值的自动压实，可选自动 GC        |
| 垃圾回收 | `gc`                     | 清理孤页、临时文件                               |
| 热点     | `hot`                    | 展示热度排名与多页主键                           |
| 事务     | `txids`                  | 查看/清理事务 ID 注册表                          |
| 导出     | `dump`                   | 导出指定顺序 + 主键的数据页                      |
| 读者     | `readers`                | 查看活跃读者及其使用的 epoch                     |
| 基准     | `bench` / `benchmark`    | 快速生成样本或执行内置基准测试                   |

## `stats`

```bash
synapsedb stats demo.synapsedb --summary
synapsedb stats demo.synapsedb --txids=20
synapsedb stats demo.synapsedb --txids-window=60
```

- `--summary`：简化输出
- `--txids[=N]`：附带最近 N 条事务 ID
- `--txids-window=MIN`：统计最近 MIN 分钟的事务数量

## `check`

```bash
synapsedb check demo.synapsedb
synapsedb check demo.synapsedb --strict
```

- 检查 manifest、索引页、WAL 校验和
- `--strict` 会逐页比对、重放 WAL，耗时更长

## `repair`

```bash
synapsedb repair demo.synapsedb --fast
synapsedb repair demo.synapsedb --rebuild-indexes
```

- `--fast`：按页修复映射
- 未指定选项时，若检测到严重问题会自动执行全量重建

## `auto-compact`

```bash
synapsedb auto-compact demo.synapsedb \
  --mode=incremental \
  --orders=SPO,POS \
  --min-merge=2 \
  --hot-threshold=1.1 \
  --max-primary=5 \
  --include-lsm-segments \
  --auto-gc
```

- `--mode`：`incremental`（默认）或 `rewrite`
- `--min-merge`：多页阈值
- `--hot-threshold`：热度阈值，大于此值的主键优先
- `--max-primary`：单次处理主键数量
- `--auto-gc`：压实后自动执行 GC

## `gc`

```bash
synapsedb gc demo.synapsedb --respect-readers
synapsedb gc demo.synapsedb --no-respect-readers   # 谨慎使用
```

- 清理 `orphans` 与无效索引页
- 默认尊重读者；关闭可能导致读一致性问题

## `txids`

```bash
synapsedb txids demo.synapsedb --list=20
synapsedb txids demo.synapsedb --since=120
synapsedb txids demo.synapsedb --session=ingest
synapsedb txids demo.synapsedb --clear
```

- 管理幂等事务 ID
- `--clear`：清空注册表，需确保业务允许重复提交

## `dump`

```bash
synapsedb dump demo.synapsedb SPO 42
synapsedb dump demo.synapsedb POS 10 --output spo-10.ndjson
```

- 导出指定主键所在页的原始数据
- 可用于调试属性、索引错乱等问题

## `bench`

```bash
synapsedb bench sample.synapsedb 500 lsm
synapsedb benchmark demo.synapsedb core
```

- `bench`：生成演示数据
- `benchmark`：执行内置性能套件（core/search/graph/spatial/regression 等）

## 命令选项通用约定

- `--json`：部分命令（如 `stats`）支持输出 JSON，便于脚本处理
- `--log-level`：`info`（默认） / `debug`
- `--dry-run`：在 `auto-compact` 等命令中预览操作

## 日志与返回值

- 成功：退出码 0
- 校验失败：退出码 2
- 其他错误：退出码 1
- 日志默认写 stdout，可通过 `> file` 重定向

## 参考

- 若 CLI 未全局安装，可执行 `pnpm exec tsx src/cli/<command>.ts`
- 更多脚本模板：`docs/使用示例/09-嵌入式脚本与自动化-示例.md`
