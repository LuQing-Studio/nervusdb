
# 数据库实例API

<cite>
**本文档中引用的文件**
- [synapseDb.ts](file://src/synapseDb.ts)
- [openOptions.ts](file://src/types/openOptions.ts)
- [persistentStore.ts](file://src/storage/persistentStore.ts)
</cite>

## 目录
1. [简介](#简介)
2. [核心组件](#核心组件)
3. [配置选项详解](#配置选项详解)
4. [事务与资源管理](#事务与资源管理)
5. [数据操作方法](#数据操作方法)
6. [节点标识与查询](#节点标识与查询)
7. [快照与隔离机制](#快照与隔离机制)
8. [实验性API](#实验性api)

## 简介
SynapseDB 是一个嵌入式三元组知识库，提供类 SQLite 的单文件数据库功能，专门用于存储和查询主语-谓语-宾语（SPO）三元组数据。该系统支持分页索引、WAL 事务、快照一致性、自动压缩和垃圾回收等高级特性。

**Section sources**
- [synapseDb.ts](file://src/synapseDb.ts#L57-L915)

## 核心组件

SynapseDB 类是数据库的核心入口点，封装了持久化存储层并提供了丰富的 API 接口。所有数据库操作都通过该类的静态或实例方法进行。

```mermaid
classDiagram
class SynapseDB {
+static open(path : string, options? : SynapseDBOpenOptions) : Promise<SynapseDB>
+addFact(fact : FactInput, options : FactOptions) : FactRecord
+deleteFact(fact : FactInput) : void
+getNodeId(value : string) : number | undefined
+getNodeValue(id : number) : string | undefined
+flush() : Promise<void>
+withSnapshot<T>(fn : (db : SynapseDB) => Promise<T> | T) : Promise<T>
+getStagingMetrics() : { lsmMemtable : number }
-store : PersistentStore
-snapshotDepth : number
}
class PersistentStore {
+static open(path : string, options : PersistentStoreOptions) : Promise<PersistentStore>
+addFact(fact : FactInput) : PersistedFact
+deleteFact(fact : FactInput) : void
+getNodeIdByValue(value : string) : number | undefined
+getNodeValueById(id : number) : string | undefined
+flush() : Promise<void>
+close() : Promise<void>
}
SynapseDB --> PersistentStore : "封装"
```

**Diagram sources**
- [synapseDb.ts](file://src/synapseDb.ts#L57-L915)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L61-L1633)

**Section sources**
- [synapseDb.ts](file://src/synapseDb.ts#L57-L915)

## 配置选项详解

### SynapseDBOpenOptions 接口
`SynapseDB.open()` 方法接受一个可选的配置对象，用于控制数据库的行为和性能特征。

| 参数 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `indexDirectory` | string | `${dbPath}.pages` | 索引目录路径，包含分页索引文件和元数据 |
| `pageSize` | number | 1000 | 每个索引页面包含的最大三元组数量（1-10000） |
| `rebuildIndexes` | boolean | false | 是否重建分页索引（用于恢复或升级） |
| `compression` | {codec: 'none' \| 'brotli', level?: number} | {codec: 'none'} | 索引页面压缩方式 |
| `enableLock` | boolean | false | 启用进程级独占写锁防止并发写入损坏 |
| `registerReader` | boolean | true | 注册为读者以避免维护任务干扰查询 |
| `stagingMode` | 'default' \| 'lsm-lite' | 'default' | 写入策略模式（'lsm-lite' 为实验性） |
| `enablePersistentTxDedupe` | boolean | false | 启用跨重启的事务ID幂等去重 |
| `maxRememberTxIds` | number | 1000 | 内存中保持的事务ID最大数量 |

#### 配置示例
```typescript
// 生产环境推荐配置
const db = await SynapseDB.open('./prod-db.synapsedb', {
  pageSize: 2000,
  enableLock: true,
  registerReader: true,
  compression: { codec: 'brotli', level: 6 },
  enablePersistentTxDedupe: true
});

// 开发环境轻量配置
const devDb = await SynapseDB.open('./dev-db.synapsedb', {
  pageSize: 500,
  enableLock: false,
  compression: { codec: 'none' }
});
```

**Section sources**
- [openOptions.ts](file://src/types/openOptions.ts#L5-L111)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L46-L59)

## 事务与资源管理

### flush() 方法
强制将所有未持久化的更改写入磁盘，确保数据的持久性保证。

```mermaid
sequenceDiagram
participant App as 应用程序
participant DB as SynapseDB
participant Store as PersistentStore
App->>DB : flush()
DB->>Store : flush()
Store-->>DB : 完成WAL提交和索引更新
DB-->>App : Promise<void>
Note over DB,Store : 确保所有写入操作已安全落盘
```

**Diagram sources**
- [synapseDb.ts](file://src/synapseDb.ts#L199-L201)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L99-L238)

### close() 方法
优雅地关闭数据库连接，释放所有资源。

```mermaid
flowchart TD
Start([开始关闭]) --> Flush["执行 flush()"]
Flush --> Unregister["注销读者注册"]
Unregister --> Release["释放文件锁"]
Release --> Cleanup["清理临时资源"]
Cleanup --> End([数据库已关闭])
```

**Diagram sources**
- [synapseDb.ts](file://src/synapseDb.ts#L472-L474)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L99-L238)

**Section sources**
- [synapseDb.ts](file://src/synapseDb.ts#L199-L201)
- [synapseDb.ts](file://src/synapseDb.ts#L472-L474)

## 数据操作方法

### addFact() 方法
添加一个新的事实记录到数据库中，并可选择性地附加属性。

**TypeScript 类型签名**
```typescript
addFact(fact: FactInput, options: FactOptions = {}): FactRecord
```

**参数约束**
- `fact`: 必需，包含 subject、predicate、object 字符串字段的对象
- `options`: 可选，包含 subjectProperties、objectProperties、edgeProperties 的对象

**异常抛出条件**
- 当数据库处于只读模式时尝试写入
- 当 WAL 日志无法写入时

**返回值结构**
返回包含原始事实和所有相关属性的完整 `FactRecord` 对象。

```mermaid
flowchart TD
Start([addFact调用]) --> Validate["验证输入参数"]
Validate --> AddTriple["向TripleStore添加三元组"]
AddTriple --> SetSubjectProps{"设置主语属性?"}
SetSubjectProps --> |是| UpdateSubject["调用setNodeProperties"]
SetSubjectProps --> |否| SetObjectProps{"设置宾语属性?"}
UpdateSubject --> SetObjectProps
SetObjectProps --> |是| UpdateObject["调用setNodeProperties"]
SetObjectProps --> |否| SetEdgeProps{"设置边属性?"}
UpdateObject --> SetEdgeProps
SetEdgeProps --> |是| UpdateEdge["调用setEdgeProperties"]
SetEdgeProps --> |否| Resolve["解析完整记录"]
UpdateEdge --> Resolve
Resolve --> Return["返回FactRecord"]
```

**Diagram sources**
- [synapseDb.ts](file://src/synapseDb.ts#L101-L131)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L99-L238)

### deleteFact() 方法
从数据库中删除指定的事实记录。

**TypeScript 类型签名**
```typescript
deleteFact(fact: FactInput): void
```

**行为说明**
-