# 02 · 项目接入（npm link 开发联调）

> 适合本地双仓联调：在 SynapseDB 仓库 link，一次更改，多处引用。

## 1）在 SynapseDB 仓库执行 link

```bash
npm link
```

## 2）在你的业务项目中 link 进来

```bash
npm link synapsedb
```

## 3）使用方式同“本地 tgz”示例

- import：`import { SynapseDB } from 'synapsedb'`
- 运行脚本同上

## 4）还原

```bash
# 在业务项目
npm unlink synapsedb && npm i
# 在 SynapseDB 仓库
npm unlink
```
