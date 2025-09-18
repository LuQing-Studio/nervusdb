# 00 · 全局 CLI 快速开始

已全局安装后可直接使用 `synapsedb`。以下命令可在任意目录执行。

## 生成样本库

```bash
synapsedb bench demo.synapsedb 100 lsm
```

- 生成 100 条记录，并演示 LSM-Lite 暂存

## 查看统计

```bash
synapsedb stats demo.synapsedb
synapsedb stats demo.synapsedb --txids=50 --txids-window=30
```

## 自动合并（增量）+ 自动 GC

```bash
synapsedb auto-compact demo.synapsedb \
  --mode=incremental --orders=SPO --min-merge=2 \
  --hot-threshold=1 --max-primary=1 --auto-gc
```

## 页面级 GC

```bash
synapsedb gc demo.synapsedb
```

## 自检与修复

```bash
synapsedb check demo.synapsedb --summary
synapsedb check demo.synapsedb --strict
synapsedb repair demo.synapsedb --fast
```

## 导出页内容（调试）

```bash
synapsedb dump demo.synapsedb SPO 1
```
