# 01 · 项目接入（本地 tgz 安装）

> 适用于“尚未发布到 npm / 私有源”的本地验证；推荐方式。

## 1）在 SynapseDB 仓库打包

```bash
pnpm build && pnpm pack
# 生成 synapsedb-<version>.tgz
```

## 2）在你的业务项目中安装 tgz

```bash
# 进入你的业务项目根目录
npm i ../path/to/synapsedb-1.0.0.tgz
```

## 3）最小示例（ESM）

新建 `scripts/demo.mts`：

```ts
import { SynapseDB } from 'synapsedb';

const db = await SynapseDB.open('mydb.synapsedb', {
  pageSize: 1024,
  enableLock: true,
  registerReader: true,
  compression: { codec: 'brotli', level: 4 },
});

db.addFact({ subject: 'Alice', predicate: 'KNOWS', object: 'Bob' });

db.addFact({ subject: 'Bob', predicate: 'KNOWS', object: 'Carol' });

const friends = db.find({ subject: 'Alice', predicate: 'KNOWS' }).follow('KNOWS').all();

console.log(
  'Alice 的二跳朋友：',
  friends.map((x) => x.object),
);

await db.flush();
await db.close();
```

运行（任选其一）：

```bash
npx tsx scripts/demo.mts
# 或
node --loader ts-node/esm scripts/demo.mts
```

## 4）注意事项

- 你的项目需为 ESM（package.json: `{ "type": "module" }`）或使用 ESM 运行器
- 生产建议 `enableLock: true`（写锁）与 `registerReader: true`
- 写入后 `await db.flush()` 保证持久化与索引合并
