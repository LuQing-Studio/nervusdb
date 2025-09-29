# API 参考

<cite>
**本文档中引用的文件**   
- [synapseDb.ts](file://src/synapseDb.ts) - *核心数据库类与主API入口*
- [index.ts](file://src/index.ts) - *模块导出与架构整合*
- [queryBuilder.ts](file://src/query/queryBuilder.ts) - *查询构建器与惰性执行引擎*
- [typedSynapseDb.ts](file://src/typedSynapseDb.ts) - *类型安全数据库接口*
- [types/enhanced.ts](file://src/types/enhanced.ts) - *增强类型定义*
- [query/cypher.ts](file://src/query/cypher.ts) - *Cypher查询语言支持*
- [types/openOptions.ts](file://src/types/openOptions.ts) - *数据库打开选项配置*
- [docs/教学文档/附录-API-查询与惰性执行.md](file://docs/教学文档/附录-API-查询与惰性执行.md) - *新增的惰性执行与explain API文档*
</cite>

## 更新摘要
**变更内容**   
- **新增API附录说明**：根据 `1a0a0c5` 提交，新增关于惰性执行和 `explain()` API 的详细说明
- **移除GraphQL支持**：根据 `1e4ed1f` 提交，已移除GraphQL查询引擎，相关API已废弃
- **增强explain估算能力**：根据 `ad9cf54` 提交，`explain()` 方法现在输出 `estimatedOutput` 字段，用于估算输出基数
- **稳定explain类型**：根据 `7a23e93` 提交，修正了 `explain()` 的类型定义，确保类型安全

## 目录
1. [核心数据库类 SynapseDB](#核心数据库类-synapsedb)
2. [查询构建器 QueryBuilder](#查询构建器-querybuilder)
3. [类型安全数据库 TypedSynapseDB](#类型安全数据库-typedsynapsedb)
4. [高级查询语言支持](#高级查询语言支持)
5. [事务与快照API](#事务与快照api)
6. [接口定义摘要](#接口定义摘要)

## 核心数据库类 SynapseDB

`SynapseDB` 是嵌入式三元组知识库的核心类，提供打开、关闭数据库以及增删查改事实记录的功能。

### 打开与关闭数据库
`open` 方法用于打开或创建一个数据库实例。如果指定路径的数据库文件不存在，则会自动创建。

```typescript
const db = await SynapseDB.open('./my-database.synapsedb');
```

`close` 方法用于关闭数据库连接并释放资源。

```typescript
await db.close();
```

**节来源**
- [synapseDb.ts](file://src/synapseDb.ts#L84-L108)

### 添加和删除事实
`addFact` 方法用于向数据库中添加一个新的事实（SPO三元组），并可选择性地为节点和边设置属性。

```typescript
db.addFact(
  { subject: 'Alice', predicate: 'knows', object: 'Bob' },
  {
    subjectProperties: { age: 30 },
    edgeProperties: { since: new Date() }
  }
);
```

`deleteFact` 方法根据给定的事实描述删除对应的记录。

```typescript
db.deleteFact({ subject: 'Alice', predicate: 'knows', object: 'Bob' });
```

**节来源**
- [synapseDb.ts](file://src/synapseDb.ts#L117-L147)
- [synapseDb.ts](file://src/synapseDb.ts#L363-L365)

### 基本查询操作
`find` 方法根据条件查找匹配的事实记录，并返回一个 `QueryBuilder` 实例以支持链式调用。**注意**：根据最新代码，`find` 方法默认返回 `LazyQueryBuilder` 实例，实现惰性执行。

```typescript
const results = db.find({ predicate: 'knows' }).all();
```

`findStreaming` 方法提供真正的流式查询能力，适用于处理大数据集，避免内存溢出。

```typescript
const stream = await db.findStreaming({ predicate: 'knows' });
for await (const fact of stream) {
  console.log(fact);
}
```

`findByNodeProperty` 和 `findByEdgeProperty` 分别基于节点或边的属性进行查询。

```typescript
// 查找年龄为25的用户
const users = db.findByNodeProperty({ propertyName: 'age', value: 25 }).all();

// 查找权重为0.8的关系
const strongRelations = db.findByEdgeProperty({ propertyName: 'weight', value: 0.8 }).all();
```

`findByLabel` 方法根据节点标签进行查询。

```typescript
const persons = db.findByLabel('Person').all();
```

**节来源**
- [synapseDb.ts](file://src/synapseDb.ts#L206-L216)
- [synapseDb.ts](file://src/synapseDb.ts#L228-L243)

### 数据库打开选项
`SynapseDBOpenOptions` 接口提供了丰富的数据库配置选项：

```typescript
interface SynapseDBOpenOptions {
  /**
   * 索引目录路径
   * @default `${dbPath}.pages`
   */
  indexDirectory?: string;

  /**
   * 页面大小（三元组数量）
   * @default 1000
   */
  pageSize?: number;

  /**
   * 是否重建索引
   * @default false
   */
  rebuildIndexes?: boolean;

  /**
   * 压缩选项
   * @default { codec: 'none' }
   */
  compression?: {
    codec: 'none' | 'brotli';
    level?: number;
  };

  /**
   * 启用进程级独占写锁
   * @default false
   */
  enableLock?: boolean;

  /**
   * 注册为读者
   * @default true（自 v2 起）
   */
  registerReader?: boolean;

  /**
   * 暂存模式
   * @default 'default'
   */
  stagingMode?: 'default' | 'lsm-lite';

  /**
   * 启用跨周期 txId 幂等去重
   * @default false
   */
  enablePersistentTxDedupe?: boolean;

  /**
   * 记忆的最大事务 ID 数量
   * @default 1000
   */
  maxRememberTxIds?: number;

  /**
   * 实验性功能开关
   */
  experimental?: {
    /** 是否启用 Cypher 查询语言插件 */
    cypher?: boolean;
    /** 是否启用 Gremlin 查询语言辅助工厂 */
    gremlin?: boolean;
    /** 是否启用 GraphQL 查询语言辅助工厂 */
    graphql?: boolean;
  };
}
```

这些选项允许精细控制数据库的行为、性能和并发特性。

**节来源**
- [types/openOptions.ts](file://src/types/openOptions.ts#L5-L126)

## 查询构建器 QueryBuilder

`QueryBuilder` 提供了链式的查询构造能力，允许通过一系列方法组合来精确控制查询结果。

### 联想查询
`follow` 和 `followReverse` 方法分别实现正向和反向的联想查询。

```typescript
// 查找 Alice 的朋友的朋友
const friendsOfFriends = db.find({ subject: 'Alice' })
  .follow('knows')
  .follow('knows')
  .all();
```

### 条件过滤
`where` 方法接受一个谓词函数，对当前结果集中的每条记录进行过滤。

```typescript
// 过滤出年龄大于25的用户
const adults = db.findByNodeProperty({ propertyName: 'age' })
  .where(record => (record.subjectProperties?.age as number) > 25)
  .all();
```

### 结果限制
`limit` 和 `skip` 方法用于分页或限制返回的结果数量。

```typescript
// 获取前10条记录
const top10 = db.find({}).limit(10).all();

// 跳过前5条记录
const page2 = db.find({}).skip(5).limit(5).all();
```

### 属性与标签过滤
`whereProperty` 方法支持基于节点或边属性的等值或范围查询。

```typescript
// 查找年龄在25到35之间的用户
const rangeQuery = db.find({})
  .whereProperty('age', '>=', 25, 'node')
  .whereProperty('age', '<=', 35, 'node');
```

`whereLabel` 方法根据节点标签进一步筛选结果。

```typescript
// 筛选出同时具有 Person 和 Employee 标签的主体
const employees = db.find({})
  .whereLabel(['Person', 'Employee'], { mode: 'AND', on: 'subject' });
```

### 流式查询与惰性执行
`LazyQueryBuilder` 和 `StreamingQueryBuilder` 支持惰性执行和流式处理，避免大结果集的内存压力。

```typescript
// 惰性查询，仅在调用 all() 时执行
const lazyQuery = db.find({ predicate: 'knows' }).limit(100);

// 流式查询，逐条处理结果
const stream = await db.findStreaming({ predicate: 'knows' });
for await (const fact of stream) {
  // 处理单条记录
}
```

### 查询诊断
`explain()` 方法提供查询执行计划的诊断信息。**更新**：现在包含 `estimatedOutput` 字段，用于估算输出基数。

```typescript
const plan = db.find({ predicate: 'knows' })
  .follow('worksAt')
  .limit(10)
  .explain();

console.log(plan.estimate?.estimatedOutput); // 估算的输出数量
```

**节来源**
- [queryBuilder.ts](file://src/query/queryBuilder.ts#L42-L877)
- [queryBuilder.ts](file://src/query/queryBuilder.ts#L979-L1941)
- [queryBuilder.ts](file://src/query/queryBuilder.ts#L882-L971)
- [queryBuilder.ts](file://src/query/queryBuilder.ts#L170-L178)

## 类型安全数据库 TypedSynapseDB

`TypedSynapseDB` 接口提供了泛型化的类型安全访问方式，确保在编译期就能捕获属性类型错误。

### 泛型绑定示例
通过泛型参数可以定义节点和边的属性结构。

```typescript
interface PersonNode {
  name: string;
  age?: number;
  email?: string;
}

interface RelationshipEdge {
  since: Date;
  strength: number;
}

const typedDb = await TypedSynapseDB.open<PersonNode, RelationshipEdge>('./social.db');
```

### 类型安全查询的优势
使用 `TypedSynapseDB` 后，所有涉及属性的操作都具备完整的类型检查。

```typescript
// 编译时即可发现拼写错误或类型不匹配
const result = typedDb.find({ subject: 'Alice' })
  .where(record => record.subjectProperties.age > 30) // ✅ 类型正确
  .all();
```

这避免了运行时因属性名错误或类型不符导致的问题，提升了开发效率和代码可靠性。

**节来源**
- [typedSynapseDb.ts](file://src/typedSynapseDb.ts#L0-L291)
- [types/enhanced.ts](file://src/types/enhanced.ts#L141-L215)

## 高级查询语言支持

### Cypher 查询
`cypherQuery` 方法执行标准的 Cypher 查询语句，支持参数化和只读模式。

```typescript
const result = await db.cypherQuery(
  'MATCH (a:Person)-[r:KNOWS]->(b:Person) WHERE a.age > $minAge RETURN a, b',
  { minAge: 18 }
);
```

`validateCypher` 方法可用于验证 Cypher 语句的语法正确性。

```typescript
const validation = db.validateCypher('MATCH (n) RETURN n');
if (!validation.valid) {
  console.error(validation.errors);
}
```

### GraphQL 查询
**已移除**：根据 `1e4ed1f` 提交，GraphQL查询引擎已被移除，相关API已废弃。

### Gremlin 查询
**已移除**：根据 `1e4ed1f` 提交，Gremlin查询引擎已被移除，相关API已废弃。

**节来源**
- [query/cypher.ts](file://src/query/cypher.ts#L577-L583)
- [plugins/cypher.ts](file://src/plugins/cypher.ts#L21-L175)

## 事务与快照API

### 事务批次控制
`SynapseDB` 支持显式的事务批次控制，允许将多个写入操作组合成一个原子批次。

#### 开始批次
`beginBatch` 方法开始一个新的事务批次，可选地指定 `txId` 和 `sessionId` 用于幂等性控制。

```typescript
db.beginBatch({ 
  txId: 'transaction-001', 
  sessionId: 'writer-instance-A' 
});
```

#### 提交批次
`commitBatch` 方法提交当前批次的所有更改。可通过 `durable` 选项控制持久性保证。

```typescript
// 提交并确保数据持久化到磁盘
db.commitBatch({ durable: true });
```

#### 中止批次
`abortBatch` 方法回滚当前批次的所有更改。

```typescript
// 如果发生错误，回滚所有更改
try {
  db.beginBatch();
  db.addFact({ subject: 'A', predicate: 'R', object: 'B' });
  db.setNodeProperties(nodeId, { status: 'active' });
  db.commitBatch();
} catch (error) {
  db.abortBatch(); // 回滚所有更改
  throw error;
}
```

**节来源**
- [synapseDb.ts](file://src/synapseDb.ts#L460-L470)

### 读快照一致性
`withSnapshot` 方法确保在回调函数执行期间，数据库视图保持一致，防止因其他写入操作导致的数据漂移。

```typescript
// 在快照中执行复杂查询，保证数据一致性
await db.withSnapshot(async (snapshotDb) => {
  const aliceFriends = snapshotDb.find({ subject: 'Alice', predicate: 'knows' }).all();
  const bobFriends = snapshotDb.find({ subject: 'Bob', predicate: 'knows' }).all();
  // 这两个查询看到的是同一时刻的数据状态
});
```

**节来源**
- [synapseDb.ts](file://src/synapseDb.ts#L472-L474)

## 接口定义摘要

以下是主要接口的 TypeScript 定义摘要：

### SynapseDBOpenOptions
```typescript
interface SynapseDBOpenOptions {
  indexDirectory?: string;
  pageSize?: number;
  rebuildIndexes?: boolean;
  compression?: { codec: 'none' | 'brotli'; level?: number };
  enableLock?: boolean;
  registerReader?: boolean;
  stagingMode?: 'default' | 'lsm-lite';
  enablePersistentTxDedupe?: boolean;
  maxRememberTxIds?: number;
  experimental?: {
    cypher?: boolean;
    gremlin?: boolean;
    graphql?: boolean;
  };
}
```

### TypedSynapseDB
```typescript
interface TypedSynapseDB<TNodeProps extends NodeProperties, TEdgeProps extends EdgeProperties> {
  addFact(fact: TypedFactInput, options?: TypedFactOptions<TNodeProps, TEdgeProps>): TypedFactRecord<TNodeProps, TEdgeProps>;
  find<TCriteria extends FactCriteria>(criteria: TCriteria, options?: { anchor?: FrontierOrientation }): TypedQueryBuilder<TNodeProps, TEdgeProps, TCriteria>;
  findByNodeProperty<T>(propertyFilter: TypedPropertyFilter<T>, options?: { anchor?: FrontierOrientation }): TypedQueryBuilder<TNodeProps, TEdgeProps, FactCriteria>;
  findByEdgeProperty<T>(propertyFilter: TypedPropertyFilter<T>, options?: { anchor?: FrontierOrientation }): TypedQueryBuilder<TNodeProps, TEdgeProps, FactCriteria>;
  findByLabel(labels: string | string[], options?: { mode?: 'AND' | 'OR'; anchor?: FrontierOrientation }): TypedQueryBuilder<TNodeProps, TEdgeProps, FactCriteria>;
  getNodeProperties(nodeId: number): TNodeProps | null;
  getEdgeProperties(key: { subjectId: number; predicateId: number; objectId: number }): TEdgeProps | null;
  setNodeProperties(nodeId: number, properties: TNodeProps): void;
  setEdgeProperties(key: { subjectId: number; predicateId: number; objectId: number }, properties: TEdgeProps): void;
  flush(): Promise<void>;
  close(): Promise<void>;
}
```

**节来源**
- [types/openOptions.ts](file://src/types/openOptions.ts#L5-L126)
- [types/enhanced.ts](file://src/types/enhanced.ts#L141-L215)