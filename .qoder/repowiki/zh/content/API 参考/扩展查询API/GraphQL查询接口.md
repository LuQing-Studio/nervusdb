# GraphQL查询接口

<cite>
**本文档引用文件**  
- [processor.ts](file://src/query/graphql/processor.ts)
- [discovery.ts](file://src/query/graphql/discovery.ts)
- [builder.ts](file://src/query/graphql/builder.ts)
- [types.ts](file://src/query/graphql/types.ts)
- [index.ts](file://src/query/graphql/index.ts)
</cite>

## 目录
1. [介绍](#介绍)
2. [核心组件分析](#核心组件分析)
3. [Schema自动发现机制](#schema自动发现机制)
4. [查询处理流程](#查询处理流程)
5. [类型映射与元数据构建](#类型映射与元数据构建)
6. [嵌套关系遍历优化](#嵌套关系遍历优化)
7. [分页与过滤实现](#分页与过滤实现)
8. [别名与联合类型支持](#别名与联合类型支持)
9. [自定义标量类型扩展](#自定义标量类型扩展)

## 介绍
SynapseDB的GraphQL查询接口提供了一套完整的动态Schema生成和查询执行能力，能够从知识图谱中自动推断类型结构并生成相应的API。该系统支持查询优化、缓存和批量加载等高级特性。

## 核心组件分析

### GraphQL处理器架构
```mermaid
classDiagram
class GraphQLProcessor {
+store : PersistentStore
-schema : GeneratedSchema
-entityTypes : EntityTypeInfo[]
-dataLoaders : Map<string, DataLoader>
+initialize() : Promise<GeneratedSchema>
+executeQuery(query : string, variables : Record, context : any) : Promise<GraphQLExecutionResult>
+parseQuery(query : string) : ParsedGraphQLQuery
+executeOperation(parsedQuery : ParsedGraphQLQuery, variables : Record, context : GraphQLContext) : Promise<Record>
}
class GraphQLService {
+store : PersistentStore
-processor : GraphQLProcessor
-validator : GraphQLValidator
-initialized : boolean
+initialize() : Promise<void>
+getSchema() : Promise<string>
+executeQuery(query : string, variables? : Record, context? : any) : Promise<any>
+validateQuery(query : string) : Promise<any[]>
}
class SchemaDiscovery {
+store : PersistentStore
+config : Required<SchemaGenerationConfig>
+discoverEntityTypes() : Promise<EntityTypeInfo[]>
+inferStructuralTypes(records : any[], existingTypes : Map) : Promise<Map>
+analyzeTypeProperties(typeInfo : EntityTypeInfo, entityTypeMap : Map) : Promise<void>
+analyzeTypeRelations(typeInfo : EntityTypeInfo, entityTypeMap : Map) : Promise<void>
}
class SchemaBuilder {
+store : PersistentStore
+config : Required<SchemaGenerationConfig>
+resolverOptions : ResolverGenerationOptions
+buildSchema(entityTypes : EntityTypeInfo[]) : Promise<GeneratedSchema>
+generateTypes(entityTypes : EntityTypeInfo[]) : Promise<GraphQLType[]>
+generateRootTypes(entityTypes : EntityTypeInfo[]) : Promise<GraphQLType[]>
+generateResolvers(entityTypes : EntityTypeInfo[], types : GraphQLType[]) : Promise<Record>
}
GraphQLService --> GraphQLProcessor : "使用"
GraphQLProcessor --> SchemaDiscovery : "实例化"
GraphQLProcessor --> SchemaBuilder : "实例化"
SchemaBuilder --> SchemaDiscovery : "输入"
```

**图表来源**
- [processor.ts](file://src/query/graphql/processor.ts#L0-L59)
- [index.ts](file://src/query/graphql/index.ts#L29-L50)
- [discovery.ts](file://src/query/graphql/discovery.ts#L21-L479)
- [builder.ts](file://src/query/graphql/builder.ts#L77-L102)

**本节来源**
- [processor.ts](file://src/query/graphql/processor.ts#L0-L638)
- [index.ts](file://src/query/graphql/index.ts#L29-L331)

## Schema自动发现机制

### 实体类型发现流程
```mermaid
flowchart TD
Start([开始]) --> CollectRecords["收集所有记录"]
CollectRecords --> FindExplicitTypes["查找明确的类型声明<br/>如TYPE, rdf:type等"]
FindExplicitTypes --> InferStructuralTypes["基于结构模式推断类型<br/>通过谓词签名聚类"]
InferStructuralTypes --> MergeTypeInfo["合并类型信息"]
MergeTypeInfo --> AnalyzeProperties["分析每个类型的属性"]
AnalyzeProperties --> AnalyzeRelations["分析每个类型的关系"]
AnalyzeRelations --> FilterAndSort["过滤并按数量排序"]
FilterAndSort --> End([返回实体类型数组])
```

**图表来源**
- [discovery.ts](file://src/query/graphql/discovery.ts#L21-L479)

**本节来源**
- [discovery.ts](file://src/query/graphql/discovery.ts#L21-L479)

## 查询处理流程

### GraphQL查询执行序列
```mermaid
sequenceDiagram
participant Client as "客户端"
participant Service as "GraphQLService"
participant Processor as "GraphQLProcessor"
participant Discovery as "SchemaDiscovery"
participant Builder as "SchemaBuilder"
participant Store as "PersistentStore"
Client->>Service : executeQuery(query)
Service->>Service : initialize()
Service->>Processor : initialize()
Processor->>Discovery : discoverEntityTypes()
Discovery-->>Processor : entityTypes
Processor->>Builder : buildSchema(entityTypes)
Builder-->>Processor : schema
Processor-->>Service : schema
Service->>Processor : executeQuery(query)
Processor->>Processor : parseQuery(query)
Processor->>Processor : executeOperation()
loop 每个字段
Processor->>Processor : resolveField()
alt 存在解析器
Processor->>Resolver : 调用对应解析器
Resolver-->>Processor : 返回结果
else 无解析器
Processor->>Store : 默认字段解析
Store-->>Processor : 属性值或关系
end
end
Processor-->>Service : 执行结果
Service-->>Client : {data, errors}
```

**图表来源**
- [processor.ts](file://src/query/graphql/processor.ts#L107-L134)
- [index.ts](file://src/query/graphql/index.ts#L124-L150)

**本节来源**
- [processor.ts](file://src/query/graphql/processor.ts#L107-L338)
- [index.ts](file://src/query/graphql/index.ts#L124-L150)

## 类型映射与元数据构建

### 值类型推断逻辑
```mermaid
flowchart TD
Start([开始]) --> CheckEmpty["检查值数组是否为空"]
CheckEmpty --> |是| ReturnString["返回String类型"]
CheckEmpty --> |否| CountTypes["统计各类型出现次数<br/>string, number, boolean, object"]
CountTypes --> FindMaxType["找出最常见类型"]
FindMaxType --> DecideType{"判断具体类型"}
DecideType --> |number| CheckFloat["检查是否存在小数"]
CheckFloat --> |是| ReturnFloat["返回Float类型"]
CheckFloat --> |否| ReturnInt["返回Int类型"]
DecideType --> |boolean| ReturnBoolean["返回Boolean类型"]
DecideType --> |object| ReturnJSON["返回JSON类型"]
DecideType --> |其他| ReturnString["返回String类型"]
ReturnFloat --> End([结束])
ReturnInt --> End
ReturnBoolean --> End
ReturnJSON --> End
ReturnString --> End
```

**图表来源**
- [discovery.ts](file://src/query/graphql/discovery.ts#L385-L418)

**本节来源**
- [discovery.ts](file://src/query/graphql/discovery.ts#L385-L418)
- [types.ts](file://src/query/graphql/types.ts#L1-L256)

## 嵌套关系遍历优化

### 深度查询优化策略
```mermaid
graph TD
A[原始查询] --> B{检测查询深度}
B --> |深度≤maxDepth| C[直接执行查询]
B --> |深度>maxDepth| D[应用查询优化]
D --> E[启用DataLoader批量加载]
E --> F[启用解析器缓存]
F --> G[限制最大查询复杂度]
G --> H[执行优化后的查询]
C --> I[返回查询结果]
H --> I
I --> J{结果是否超时}
J --> |是| K[返回部分结果+警告]
J --> |否| L[正常返回完整结果]
```

**图表来源**
- [processor.ts](file://src/query/graphql/processor.ts#L288-L331)
- [index.ts](file://src/query/graphql/index.ts#L295-L302)

**本节来源**
- [processor.ts](file://src/query/graphql/processor.ts#L288-L331)
- [index.ts](file://src/query/graphql/index.ts#L295-L302)

## 分页与过滤实现

### 分页参数处理逻辑
```mermaid
flowchart TD
Start([开始]) --> CheckPagination["检查是否启用分页"]
CheckPagination --> |否| ReturnArray["返回普通数组"]
CheckPagination --> |是| ExtractArgs["提取分页参数<br/>first/after/last/before"]
ExtractArgs --> CalculateRange["计算起始和结束索引"]
CalculateRange --> ApplyCursorLogic["应用游标逻辑"]
ApplyCursorLogic --> |after存在| SetStartIndex["设置startIndex = afterIndex + 1"]
ApplyCursorLogic --> |before存在| SetEndIndex["设置endIndex = beforeIndex"]
SetStartIndex --> ApplyLimit
SetEndIndex --> ApplyLimit
ApplyLimit --> |first存在| LimitForward["向前限制数量"]
ApplyLimit --> |last存在| LimitBackward["向后限制数量"]
LimitForward --> SliceItems["切片获取指定范围项"]
LimitBackward --> SliceItems
SliceItems --> CreateEdges["创建Edge对象数组<br/>包含node和cursor"]
CreateEdges --> CreatePageInfo["创建PageInfo对象"]
CreatePageInfo --> AssembleConnection["组装Connection对象"]
AssembleConnection --> End([返回Connection格式结果])
```

**图表来源**
- [builder.ts](file://src/query/graphql/builder.ts#L791-L834)

**本节来源**
- [builder.ts](file://src/query/graphql/builder.ts#L791-L834)
- [index.ts](file://src/query/graphql/index.ts#L61-L113)

## 别名与联合类型支持

### 字段别名处理流程
```mermaid
flowchart LR
ParseQuery["解析查询语句"] --> ExtractFields["提取字段列表"]
ExtractFields --> ProcessField["处理每个字段"]
ProcessField --> CheckAlias{"检查是否有别名"}
CheckAlias --> |有别名| UseAlias["使用alias作为结果键"]
CheckAlias --> |无别名| UseName["使用name作为结果键"]
UseAlias --> ResolveValue["解析字段值"]
UseName --> ResolveValue
ResolveValue --> StoreResult["将结果存储到对应键"]
StoreResult --> NextField["处理下一个字段"]
NextField --> |完成| ReturnResult["返回最终结果对象"]
```

**本节来源**
- [processor.ts](file://src/query/graphql/processor.ts#L298-L338)
- [types.ts](file://src/query/graphql/types.ts#L185-L195)

## 自定义标量类型扩展

### 标量类型注册机制
```mermaid
classDiagram
class ScalarType {
+name : string
+serialize(value : unknown) : unknown
+parseValue(value : unknown) : unknown
+parseLiteral(ast : any) : unknown
}
class JSONScalar {
+serialize(value : unknown) : unknown
+parseValue(value : unknown) : unknown
+parseLiteral(ast : any) : unknown
}
class DateTimeScalar {
+serialize(value : unknown) : string
+parseValue(value : unknown) : Date
+parseLiteral(ast : any) : Date
}
class CustomGeoScalar {
+serialize(value : unknown) : string
+parseValue(value : unknown) : GeoPoint
+parseLiteral(ast : any) : GeoPoint
}
ScalarType <|-- JSONScalar
ScalarType <|-- DateTimeScalar
ScalarType <|-- CustomGeoScalar
class SchemaBuilder {
-resolvers : Record
+generateResolvers() : Promise<Record>
}
SchemaBuilder --> JSONScalar : "注册"
SchemaBuilder --> DateTimeScalar : "注册"
SchemaBuilder --> CustomGeoScalar : "可扩展注册"
```

**本节来源**
- [builder.ts](file://src/query/graphql/builder.ts#L77-L102)
- [types.ts](file://src/query/graphql/types.ts#L1-L256)