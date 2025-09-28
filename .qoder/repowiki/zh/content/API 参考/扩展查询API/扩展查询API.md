# 扩展查询API

<cite>
**本文档引用文件**   
- [synapseDb.ts](file://src/synapseDb.ts)
- [cypher.ts](file://src/query/cypher.ts)
- [aggregation.ts](file://src/query/aggregation.ts)
- [pattern/index.ts](file://src/query/pattern/index.ts)
- [pattern/compiler.ts](file://src/query/pattern/compiler.ts)
- [pattern/match.ts](file://src/query/pattern/match.ts)
- [path/variable.ts](file://src/query/path/variable.ts)
- [path/bidirectional.ts](file://src/query/path/bidirectional.ts)
- [path/astar.ts](file://src/query/astar.ts)
- [graphql/index.ts](file://src/query/graphql/index.ts)
- [graphql/builder.ts](file://src/query/graphql/builder.ts)
- [graphql/discovery.ts](file://src/query/graphql/discovery.ts)
- [gremlin/index.ts](file://src/query/gremlin/index.ts)
- [gremlin/source.ts](file://src/query/gremlin/source.ts)
</cite>

## 目录
1. [多语言查询接口概述](#多语言查询接口概述)
2. [Cypher查询语言支持](#cypher查询语言支持)
3. [GraphQL查询语言支持](#graphql查询语言支持)
4. [Gremlin查询语言支持](#gremlin查询语言支持)
5. [路径查找算法API](#路径查找算法api)
6. [聚合管道与模式匹配](#聚合管道与模式匹配)

## 多语言查询接口概述

SynapseDB提供对Cypher、GraphQL和Gremlin三种主流图查询语言的全面支持，允许开发者根据应用场景选择最适合的查询方式。系统通过统一的存储引擎为不同查询语言提供一致的数据访问能力，同时保持各语言特有的表达优势。

**Section sources**
- [synapseDb.ts](file://src/synapseDb.ts#L805-L896)
- [cypher.ts](file://src/query/cypher.ts#L222-L240)

## Cypher查询语言支持

### 同步cypher()极简实现

`cypher()`方法提供了一个极简的同步Cypher查询接口，仅支持最基础的MATCH-RETURN语法模式。该实现直接在内存中解析和执行查询，适用于简单场景下的快速数据检索。

```mermaid
flowchart TD
A["cypher(query: string)"] --> B{正则匹配}
B --> |成功| C[提取变量和关系]
C --> D{是否变长路径}
D --> |否| E[执行单跳查询]
D --> |是| F[构建VariablePathBuilder]
F --> G[执行所有路径查询]
G --> H[返回结果集]
```

**Diagram sources **
- [synapseDb.ts](file://src/synapseDb.ts#L805-L859)

### 异步cypherQuery/cypherRead标准接口

`cypherQuery`和`cypherRead`提供了完整的异步Cypher查询能力，基于词法分析、语法解析、编译执行的完整处理流程。两者主要区别在于：

- `cypherQuery`: 通用查询接口，可执行读写操作
- `cypherRead`: 只读查询接口，确保查询不会修改数据

两种接口均返回`CypherResult`对象，包含记录集、执行摘要和统计信息。

```mermaid
sequenceDiagram
participant Client as "客户端"
participant SynapseDB as "SynapseDB"
participant Processor as "CypherProcessor"
Client->>SynapseDB : cypherQuery(statement, parameters)
SynapseDB->>Processor : execute(statement, parameters)
Processor->>Processor : 词法分析(tokenize)
Processor->>Processor : 语法解析(parseTokens)
Processor->>Processor : 查询分析(analyzeStatementType)
Processor->>Processor : 编译执行(compile & execute)
Processor-->>SynapseDB : CypherResult
SynapseDB-->>Client : 返回结果
```

**Diagram sources **
- [synapseDb.ts](file://src/synapseDb.ts#L869-L888)
- [cypher.ts](file://src/query/cypher.ts#L222-L235)

### 参数绑定与只读执行

Cypher接口支持参数化查询，通过`parameters`参数传递参数值，有效防止SQL注入并提高查询性能。`cypherRead`方法通过设置`readonly: true`选项强制只读执行，当检测到写操作时会抛出异常。

**Section sources**
- [cypher.ts](file://src/query/cypher.ts#L222-L235)
- [cypher.ts](file://src/query/cypher.ts#L240-L240)

### 缓存管理机制

系统实现了查询优化器缓存，可通过以下方法进行管理：
- `clearOptimizationCache()`: 清理查询计划缓存
- `getOptimizerStats()`: 获取优化器统计信息
- `warmUpOptimizer()`: 预热查询优化器

这些方法有助于在生产环境中优化查询性能和资源使用。

**Section sources**
- [cypher.ts](file://src/query/cypher.ts#L240-L240)

### validateCypher语法验证工具

`validateCypher`方法提供独立的Cypher语法验证功能，可用于前端输入验证或查询预检。该方法返回包含`valid`布尔值和`errors`字符串数组的对象，便于集成到各种开发工作流中。

```mermaid
flowchart TD
A["validateCypher(statement)"] --> B[创建Lexer]
B --> C[分词(tokenize)]
C --> D{是否有错误}
D --> |是| E[捕获异常]
E --> F[返回{valid: false, errors}]
D --> |否| G[创建Parser]
G --> H[解析tokens]
H --> I{是否有错误}
I --> |是| J[捕获异常]
J --> K[返回{valid: false, errors}]
I --> |否| L[返回{valid: true, errors: []}]
```

**Diagram sources **
- [synapseDb.ts](file://src/synapseDb.ts#L893-L896)
- [cypher.ts](file://src/query/cypher.ts#L240-L240)

## GraphQL查询语言支持

### graphqlQuery入口函数

`graphqlQuery`入口函数提供GraphQL查询执行能力，其处理流程包括：
1. 初始化服务（首次调用时）
2. 查询语法验证
3. 执行查询
4. 返回结果

该函数自动处理Schema生成和更新，确保始终反映最新的数据结构。

```mermaid
sequenceDiagram
participant Client as "客户端"
participant GraphQLService as "GraphQLService"
participant Validator as "GraphQLValidator"
participant Processor as "GraphQLProcessor"
Client->>GraphQLService : executeQuery(query, variables)
GraphQLService->>GraphQLService : initialize()
activate GraphQLService
GraphQLService->>Validator : validateQuery(query)
Validator-->>GraphQLService : 错误列表
alt 有错误
GraphQLService-->>Client : {errors}
else 无错误
GraphQLService->>Processor : executeQuery(query, variables)
Processor-->>GraphQLService : 结果
GraphQLService-->>Client : 返回结果
end
deactivate GraphQLService
```

**Diagram sources **
- [graphql/index.ts](file://src/query/graphql/index.ts#L178-L230)

### 类型映射规则

GraphQL Schema生成器采用以下类型映射规则：
- 字符串值 → String
- 整数值 → Int  
- 浮点数值 → Float  
- 布尔值 → Boolean
- 对象值 → JSON

属性名称转换支持camelCase和snake_case等命名规范，可通过配置进行调整。

```mermaid
classDiagram
class SchemaDiscovery {
+discoverEntityTypes() EntityTypeInfo[]
+inferStructuralTypes() Map<number, string>
+analyzeTypeProperties() void
+analyzeTypeRelations() void
+inferValueType() GraphQLScalarType
}
class SchemaBuilder {
+buildSchema() GeneratedSchema
+generateObjectType() GraphQLType
+generateRelationField() GraphQLField
+generatePropertyField() GraphQLField
}
SchemaDiscovery --> SchemaBuilder : "生成"
SchemaBuilder --> GeneratedSchema : "输出"
```

**Diagram sources **
- [graphql/discovery.ts](file://src/query/graphql/discovery.ts#L22-L480)
- [graphql/builder.ts](file://src/query/graphql/builder.ts#L22-L799)

## Gremlin查询语言支持

### gremlinQuery遍历执行模型

Gremlin查询通过`GraphTraversalSource`启动，构建链式遍历管道。每个步骤添加到`steps`数组中，最终由`GremlinExecutor`执行。

```mermaid
sequenceDiagram
participant Client as "客户端"
participant Source as "GraphTraversalSource"
participant Traversal as "GraphTraversal"
participant Executor as "GremlinExecutor"
Client->>Source : traversal(store)
Source->>Source : createTraversal()
Source-->>Client : GraphTraversal
Client->>Traversal : V().has('name', '张三').out('朋友')
Traversal->>Traversal : 添加步骤到steps数组
Client->>Traversal : toList()
Traversal->>Executor : execute(steps)
Executor-->>Traversal : 结果
Traversal-->>Client : 返回结果
```

**Diagram sources **
- [gremlin/source.ts](file://src/query/gremlin/source.ts#L22-L289)
- [gremlin/index.ts](file://src/query/gremlin/index.ts#L22-L283)

### 步骤处理器链

Gremlin实现采用步骤处理器链模式，每个步骤类型对应特定的处理器。执行时按顺序应用每个处理器，逐步转换结果集。

```mermaid
flowchart LR
A[VStep] --> B[HasStep]
B --> C[OutStep]
C --> D[ValuesStep]
D --> E[ToListStep]
subgraph "执行过程"
F[初始顶点集] --> G[VStep处理器]
G --> H[过滤后顶点集]
H --> I[HasStep处理器]
I --> J[匹配条件顶点]
J --> K[OutStep处理器]
K --> L[邻居顶点集]
L --> M[ValuesStep处理器]
M --> N[属性值列表]
N --> O[ToListStep处理器]
O --> P[最终结果]
end
```

**Diagram sources **
- [gremlin/source.ts](file://src/query/gremlin/source.ts#L22-L289)

## 路径查找算法API

### BFS单向搜索

`shortestPath`方法实现了基于广度优先搜索(BFS)的单向最短路径查找算法。该算法从起始节点开始逐层扩展，直到找到目标节点或达到最大跳数限制。

**参数要求：**
- `from`: 起始节点标识符
- `to`: 目标节点标识符
- `options`: 可选参数对象，包含：
  - `predicates`: 允许的关系类型数组
  - `maxHops`: 最大跳数限制（默认8）
  - `direction`: 搜索方向（'forward'|'reverse'|'both'）

**时间复杂度：** O(V + E)，其中V为顶点数，E为边数
**返回格式：** 返回`FactRecord[]`数组表示路径，若无路径则返回null

```mermaid
flowchart TD
A[初始化队列和已访问集合] --> B{队列非空且未超最大深度}
B --> |是| C[取出当前层所有节点]
C --> D[检查是否到达目标]
D --> |是| E[返回路径]
D --> |否| F[获取邻居节点]
F --> G[过滤已访问节点]
G --> H[加入新节点到队列]
H --> I[标记为已访问]
I --> J[深度+1]
J --> B
B --> |否| K[返回null]
```

**Section sources**
- [synapseDb.ts](file://src/synapseDb.ts#L513-L577)

### 双向搜索

`BidirectionalPathBuilder`类实现了双向BFS最短路径算法，从起点和终点同时开始搜索，当两个搜索前沿相遇时即找到最短路径。

**优势：** 在稀疏图中比单向BFS更高效，时间复杂度理论上可减少一半
**适用场景：** 大规模图数据中的最短路径查找

```mermaid
sequenceDiagram
participant ForwardSearch as "前向搜索"
participant BackwardSearch as "后向搜索"
participant Intersection as "交集检测"
ForwardSearch->>ForwardSearch : 初始化起点队列
BackwardSearch->>BackwardSearch : 初始化终点队列
loop 搜索循环
ForwardSearch->>ForwardSearch : 扩展当前层节点
BackwardSearch->>BackwardSearch : 扩展当前层节点
Intersection->>Intersection : 检查前向与后向访问集合的交集
alt 发现交集
Intersection-->>ForwardSearch : 返回交集节点
break 终止搜索
end
end
```

**Section sources**
- [bidirectional.ts](file://src/query/path/bidirectional.ts#L61-L146)

### Dijkstra加权路径

`AStarPathBuilder`类提供了基于A*算法的加权最短路径查找，支持多种启发式函数。

**参数要求：**
- 支持自定义启发式函数（hop, manhattan, euclidean, custom）
- 可配置边权重和启发式权重因子
- 支持节点唯一性和边唯一性约束

**时间复杂度：** O(E + V log V)，使用优先队列优化
**返回格式：** 返回`PathResult`对象，包含边列表、路径长度和起止节点ID

```mermaid
flowchart TD
A[初始化开放列表和关闭列表] --> B{开放列表非空}
B --> |是| C[选取fScore最小节点]
C --> D{已在关闭列表?}
D --> |是| B
D --> |否| E[加入关闭列表]
E --> F{到达目标且满足最小长度?}
F --> |是| G[重构路径返回]
F --> |否| H{达到最大深度?}
H --> |是| B
H --> |否| I[扩展邻居节点]
I --> J[计算tentativeGScore]
J --> K{发现更优路径?}
K --> |是| L[更新节点信息]
L --> M[加入开放列表或更新]
M --> B
B --> |否| N[返回null]
```

**Section sources**
- [astar.ts](file://src/query/path/astar.ts#L111-L232)

## 聚合管道与模式匹配

### aggregate聚合管道

`aggregate()`方法返回`AggregationPipeline`实例，用于构建数据聚合操作流水线。

**核心功能：**
- `match()`: 基于三元组条件匹配输入
- `groupBy()`: 分组字段指定
- `count()/sum()/avg()/min()/max()`: 聚合操作
- `orderBy()`: 结果排序
- `limit()`: 结果数量限制
- `executeStreaming()`: 流式执行，适合大数据集

```mermaid
flowchart LR
A[输入记录集] --> B{matchStream?}
B --> |是| C[流式批处理]
B --> |否| D[全量加载]
C --> E[增量聚合状态]
D --> F[分组收集]
E --> G[聚合计算]
F --> G
G --> H[排序]
H --> I[限制]
I --> J[返回结果]
```

**Section sources**
- [aggregation.ts](file://src/query/aggregation.ts#L17-L424)

### match模式匹配

`match()`方法返回`PatternBuilder`实例，用于构建复杂的图模式匹配查询。

**构建步骤：**
1. `node()`: 定义节点模式（别名、标签、属性）
2. `edge()`: 定义关系模式（方向、类型、别名）
3. `variable()`: 设置变长路径范围
4. `whereNodeProperty()`: 添加属性过滤条件
5. `return()`: 指定返回字段
6. `execute()`: 执行查询并返回结果

**Section sources**
- [match.ts](file://src/query/pattern/match.ts#L21-L234)