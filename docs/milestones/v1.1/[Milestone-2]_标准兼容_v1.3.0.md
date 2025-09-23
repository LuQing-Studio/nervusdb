# [Milestone-2] 标准兼容 - v1.3.0

**版本目标**：v1.3.0
**预计时间**：2025年6月-8月（12周）
**优先级**：P1（高优先级）
**前置依赖**：Milestone-1 完成

## 🎯 里程碑概述

本里程碑专注于实现主流图数据库查询语言的兼容性，使 SynapseDB 能够支持 Cypher、Gremlin 和 GraphQL 等标准查询接口，降低用户迁移成本。

## 📋 功能清单

### 1. Cypher 查询语言支持 ⭐⭐⭐⭐⭐

#### 1.1 需求描述

实现 Neo4j Cypher 查询语言的核心子集

#### 1.2 Cypher 语法支持范围

```cypher
-- 基础查询语法
MATCH (n:Person {name: 'Alice'})-[:KNOWS]->(m:Person)
WHERE m.age > 25
RETURN n.name, m.name, m.age
ORDER BY m.age DESC
LIMIT 10

-- 创建语法
CREATE (p:Person {name: 'Bob', age: 30})
CREATE (p)-[:KNOWS {since: date('2020-01-01')}]->(q:Person {name: 'Charlie'})

-- 更新语法
MATCH (p:Person {name: 'Alice'})
SET p.age = 31
REMOVE p.temp

-- 删除语法
MATCH (p:Person {name: 'ToDelete'})
DELETE p

-- 变长路径
MATCH (a:Person {name: 'Alice'})-[:KNOWS*1..3]->(b:Person)
RETURN b

-- 聚合查询
MATCH (p:Person)-[:WORKS_AT]->(c:Company)
RETURN c.name, COUNT(p) as employee_count
ORDER BY employee_count DESC

-- 子查询
MATCH (p:Person)
WHERE EXISTS {
  MATCH (p)-[:MANAGES]->(subordinate:Person)
}
RETURN p.name
```

#### 1.3 架构设计

```typescript
// Cypher 查询处理管道
interface CypherProcessor {
  // 1. 词法分析
  lexer: CypherLexer;

  // 2. 语法分析
  parser: CypherParser;

  // 3. 语义分析
  analyzer: SemanticAnalyzer;

  // 4. 查询计划
  planner: QueryPlanner;

  // 5. 优化器
  optimizer: QueryOptimizer;

  // 6. 执行器
  executor: QueryExecutor;
}

// Cypher AST 节点
interface CypherAST {
  type: 'Query';
  clauses: Clause[];
}

interface Clause {
  type: 'MATCH' | 'CREATE' | 'SET' | 'DELETE' | 'RETURN' | 'WHERE' | 'WITH';
}

// MATCH 子句
interface MatchClause extends Clause {
  type: 'MATCH';
  optional: boolean;
  pattern: Pattern;
}

// Pattern 定义
interface Pattern {
  type: 'Path';
  elements: PathElement[];
}

interface NodePattern {
  type: 'Node';
  variable?: string;
  labels: string[];
  properties: PropertyMap;
}

interface RelationshipPattern {
  type: 'Relationship';
  variable?: string;
  types: string[];
  direction: '->' | '<-' | '-';
  properties: PropertyMap;
  varLength?: {
    min: number;
    max: number;
  };
}
```

#### 1.4 实现计划

**第1-2周：词法分析器**

```typescript
// Cypher Lexer 实现
class CypherLexer {
  private keywords = new Set([
    'MATCH',
    'CREATE',
    'SET',
    'DELETE',
    'RETURN',
    'WHERE',
    'WITH',
    'OPTIONAL',
    'UNION',
    'ORDER',
    'BY',
    'LIMIT',
    'SKIP',
    'ASC',
    'DESC',
  ]);

  tokenize(input: string): Token[] {
    const tokens: Token[] = [];
    let position = 0;

    while (position < input.length) {
      // 跳过空白
      if (this.isWhitespace(input[position])) {
        position++;
        continue;
      }

      // 识别关键字和标识符
      if (this.isLetter(input[position])) {
        const { token, newPosition } = this.readIdentifier(input, position);
        tokens.push(token);
        position = newPosition;
        continue;
      }

      // 识别字符串字面量
      if (input[position] === '"' || input[position] === "'") {
        const { token, newPosition } = this.readString(input, position);
        tokens.push(token);
        position = newPosition;
        continue;
      }

      // 识别数字
      if (this.isDigit(input[position])) {
        const { token, newPosition } = this.readNumber(input, position);
        tokens.push(token);
        position = newPosition;
        continue;
      }

      // 识别操作符
      const { token, newPosition } = this.readOperator(input, position);
      if (token) {
        tokens.push(token);
        position = newPosition;
        continue;
      }

      throw new SyntaxError(`Unexpected character: ${input[position]}`);
    }

    return tokens;
  }
}
```

**第3-4周：语法分析器**

```typescript
// 使用递归下降解析器
class CypherParser {
  private tokens: Token[];
  private position: number = 0;

  parse(tokens: Token[]): CypherAST {
    this.tokens = tokens;
    this.position = 0;

    const clauses: Clause[] = [];

    while (!this.isAtEnd()) {
      const clause = this.parseClause();
      clauses.push(clause);
    }

    return {
      type: 'Query',
      clauses,
    };
  }

  private parseClause(): Clause {
    const token = this.peek();

    switch (token.type) {
      case 'MATCH':
        return this.parseMatch();
      case 'CREATE':
        return this.parseCreate();
      case 'SET':
        return this.parseSet();
      case 'DELETE':
        return this.parseDelete();
      case 'RETURN':
        return this.parseReturn();
      case 'WHERE':
        return this.parseWhere();
      case 'WITH':
        return this.parseWith();
      default:
        throw new SyntaxError(`Unexpected token: ${token.value}`);
    }
  }

  private parseMatch(): MatchClause {
    this.consume('MATCH');

    const optional = this.check('OPTIONAL');
    if (optional) {
      this.advance();
    }

    const pattern = this.parsePattern();

    return {
      type: 'MATCH',
      optional,
      pattern,
    };
  }

  private parsePattern(): Pattern {
    const elements: PathElement[] = [];

    // 解析节点模式
    elements.push(this.parseNodePattern());

    // 解析关系和节点的链
    while (this.check('-')) {
      const relationship = this.parseRelationshipPattern();
      elements.push(relationship);

      const node = this.parseNodePattern();
      elements.push(node);
    }

    return {
      type: 'Path',
      elements,
    };
  }
}
```

**第5-6周：语义分析与类型检查**

```typescript
class SemanticAnalyzer {
  analyze(ast: CypherAST): AnalyzedAST {
    // 1. 变量作用域检查
    this.checkVariableScopes(ast);

    // 2. 类型推断
    this.inferTypes(ast);

    // 3. 语义一致性检查
    this.checkSemantics(ast);

    return {
      ...ast,
      symbolTable: this.symbolTable,
      typeInfo: this.typeInfo,
    };
  }

  private checkVariableScopes(ast: CypherAST): void {
    const scopes = new ScopeStack();

    for (const clause of ast.clauses) {
      this.checkClauseScopes(clause, scopes);
    }
  }

  private inferTypes(ast: CypherAST): void {
    // 推断节点、关系和属性的类型
    for (const clause of ast.clauses) {
      this.inferClauseTypes(clause);
    }
  }
}
```

**第7-8周：查询计划与优化**

```typescript
class CypherQueryPlanner {
  generatePlan(ast: AnalyzedAST): QueryPlan {
    // 1. 生成逻辑计划
    const logicalPlan = this.generateLogicalPlan(ast);

    // 2. 应用优化规则
    const optimizedPlan = this.optimizePlan(logicalPlan);

    // 3. 生成物理计划
    const physicalPlan = this.generatePhysicalPlan(optimizedPlan);

    return physicalPlan;
  }

  private optimizePlan(plan: LogicalPlan): LogicalPlan {
    // 优化规则
    const rules = [
      new PredicatePushdownRule(),
      new IndexSelectionRule(),
      new JoinReorderingRule(),
      new ConstantFoldingRule(),
    ];

    let optimized = plan;
    for (const rule of rules) {
      optimized = rule.apply(optimized);
    }

    return optimized;
  }
}
```

**第9-10周：执行引擎**

```typescript
class CypherExecutor {
  async execute(plan: QueryPlan, db: SynapseDB): Promise<CypherResult> {
    const context = new ExecutionContext(db);
    const operator = this.createOperator(plan.root, context);

    const results: Record<string, any>[] = [];

    await operator.open();
    try {
      while (true) {
        const tuple = await operator.next();
        if (!tuple) break;
        results.push(tuple);
      }
    } finally {
      await operator.close();
    }

    return {
      records: results,
      summary: {
        queryType: plan.queryType,
        nodesCreated: context.stats.nodesCreated,
        relationshipsCreated: context.stats.relationshipsCreated,
        propertiesSet: context.stats.propertiesSet,
      },
    };
  }
}
```

**第11-12周：集成与测试**

- [ ] Cypher API 接口实现
- [ ] 性能优化和调试
- [ ] 兼容性测试套件

#### 1.5 API 设计

```typescript
// Cypher 查询接口
interface CypherAPI {
  // 执行 Cypher 查询
  cypher(query: string, parameters?: Record<string, any>): Promise<CypherResult>;

  // 执行只读查询
  cypherRead(query: string, parameters?: Record<string, any>): Promise<CypherResult>;

  // 执行写查询
  cypherWrite(query: string, parameters?: Record<string, any>): Promise<CypherResult>;

  // 批量执行
  cypherBatch(queries: CypherQuery[]): Promise<CypherResult[]>;
}

// 扩展 SynapseDB 类
class SynapseDB implements CypherAPI {
  async cypher(query: string, parameters?: Record<string, any>): Promise<CypherResult> {
    const processor = new CypherProcessor(this);
    return await processor.execute(query, parameters);
  }
}

// 使用示例
const result = await db.cypher(
  `
  MATCH (p:Person {name: $name})-[:KNOWS]->(friend:Person)
  WHERE friend.age > $minAge
  RETURN friend.name, friend.age
  ORDER BY friend.age DESC
  LIMIT $limit
`,
  {
    name: 'Alice',
    minAge: 25,
    limit: 10,
  },
);
```

---

### 2. Gremlin 适配器 ⭐⭐⭐⭐

#### 2.1 需求描述

实现 Apache TinkerPop Gremlin 遍历语言支持

#### 2.2 Gremlin 语法支持

```javascript
// 基础遍历
g.V().hasLabel('Person').has('name', 'Alice').out('KNOWS').values('name')

// 复杂遍历
g.V().hasLabel('Person')
  .where(
    out('KNOWS').count().is(gt(5))
  )
  .project('name', 'friendCount')
  .by('name')
  .by(out('KNOWS').count())

// 聚合查询
g.V().hasLabel('Person')
  .groupCount()
  .by(values('age').map { it.get() / 10 * 10 })

// 路径查询
g.V().hasLabel('Person').has('name', 'Alice')
  .repeat(out('KNOWS')).times(3)
  .path()
```

#### 2.3 架构设计

```typescript
// Gremlin 遍历接口
interface GremlinTraversal {
  // 起始步骤
  V(ids?: string[]): GraphTraversal;
  E(ids?: string[]): GraphTraversal;

  // 过滤步骤
  has(key: string, value: any): this;
  hasLabel(...labels: string[]): this;
  where(predicate: Predicate): this;

  // 遍历步骤
  out(...edgeLabels: string[]): this;
  in(...edgeLabels: string[]): this;
  both(...edgeLabels: string[]): this;

  // 转换步骤
  values(...propertyKeys: string[]): this;
  project(...keys: string[]): this;
  by(projection: string | Traversal): this;

  // 聚合步骤
  count(): this;
  sum(): this;
  mean(): this;
  groupCount(): this;

  // 路径步骤
  path(): this;
  repeat(traversal: Traversal): this;
  times(count: number): this;

  // 终端步骤
  toList(): Promise<any[]>;
  next(): Promise<any>;
  hasNext(): Promise<boolean>;
}
```

#### 2.4 实现计划

**第13-14周：Gremlin 核心**

- [ ] 基础遍历步骤实现
- [ ] 过滤和转换步骤
- [ ] 与 SynapseDB 的适配层

**第15-16周：高级功能**

- [ ] 聚合和分组功能
- [ ] 路径遍历支持
- [ ] 性能优化

#### 2.5 API 设计

```typescript
// Gremlin 接口
interface GremlinAPI {
  g(): GremlinTraversalSource;
}

class SynapseDB implements GremlinAPI {
  g(): GremlinTraversalSource {
    return new GremlinTraversalSource(this);
  }
}

// 使用示例
const results = await db
  .g()
  .V()
  .hasLabel('Person')
  .has('name', 'Alice')
  .out('KNOWS')
  .values('name')
  .toList();
```

---

### 3. GraphQL 接口 ⭐⭐⭐

#### 3.1 需求描述

提供 GraphQL 查询接口，支持图式数据的声明式查询

#### 3.2 GraphQL Schema 设计

```graphql
# 动态生成的 GraphQL Schema
type Person {
  id: ID!
  name: String!
  age: Int
  email: String

  # 关系字段
  knows(first: Int, after: String): PersonConnection
  worksAt: Company
  manages: [Person!]!
}

type Company {
  id: ID!
  name: String!
  size: Int

  employees: [Person!]!
}

type Query {
  # 节点查询
  person(id: ID, name: String): Person
  company(id: ID, name: String): Company

  # 搜索查询
  searchPersons(query: String!, first: Int, after: String): PersonConnection

  # 路径查询
  shortestPath(from: ID!, to: ID!, maxDepth: Int = 5): [PathResult!]!

  # 聚合查询
  analytics: AnalyticsQuery
}

type AnalyticsQuery {
  personStats: PersonStats
  companyStats: CompanyStats
}

type PersonStats {
  totalCount: Int!
  averageAge: Float
  ageDistribution: [AgeGroup!]!
}

# 分页支持
type PersonConnection {
  edges: [PersonEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}
```

#### 3.3 实现计划

**第17-18周：Schema 生成**

- [ ] 动态 Schema 生成器
- [ ] 基础查询解析器
- [ ] 分页支持

**第19-20周：高级功能**

- [ ] 关系遍历优化
- [ ] 聚合查询支持
- [ ] 订阅功能（可选）

#### 3.4 API 设计

```typescript
// GraphQL 接口
interface GraphQLAPI {
  graphql(query: string, variables?: any): Promise<GraphQLResult>;
  generateSchema(): string;
}

class SynapseDB implements GraphQLAPI {
  async graphql(query: string, variables?: any): Promise<GraphQLResult> {
    const processor = new GraphQLProcessor(this);
    return await processor.execute(query, variables);
  }
}

// 使用示例
const result = await db.graphql(
  `
  query GetPersonNetwork($name: String!) {
    person(name: $name) {
      name
      age
      knows(first: 10) {
        edges {
          node {
            name
            age
          }
        }
      }
    }
  }
`,
  { name: 'Alice' },
);
```

---

## 📈 性能目标

| 功能            | 数据规模  | 目标性能 | 兼容性        |
| --------------- | --------- | -------- | ------------- |
| Cypher 基础查询 | 100万节点 | < 100ms  | Neo4j 90%     |
| Cypher 聚合查询 | 100万节点 | < 500ms  | Neo4j 80%     |
| Gremlin 遍历    | 100万节点 | < 200ms  | TinkerPop 85% |
| GraphQL 查询    | 100万节点 | < 150ms  | -             |

## 🧪 测试计划

### 兼容性测试

```typescript
describe('Cypher 兼容性', () => {
  it('支持 Neo4j Cypher 核心语法', async () => {
    const cypherQueries = [
      'MATCH (n:Person) RETURN n.name',
      'MATCH (n:Person)-[:KNOWS]->(m) WHERE m.age > 25 RETURN n, m',
      "CREATE (p:Person {name: 'Test'}) RETURN p",
      "MATCH (p:Person {name: 'Test'}) DELETE p",
    ];

    for (const query of cypherQueries) {
      const result = await db.cypher(query);
      expect(result).toBeDefined();
    }
  });
});

describe('Gremlin 兼容性', () => {
  it('支持 TinkerPop Gremlin 核心遍历', async () => {
    const result = await db
      .g()
      .V()
      .hasLabel('Person')
      .has('name', 'Alice')
      .out('KNOWS')
      .values('name')
      .toList();

    expect(result).toBeInstanceOf(Array);
  });
});
```

### 性能测试

```typescript
describe('标准查询性能', () => {
  it('Cypher 查询性能达标', async () => {
    const start = Date.now();

    await db.cypher(`
      MATCH (p:Person)-[:KNOWS]->(friend)
      WHERE friend.age > 25
      RETURN p.name, count(friend) as friendCount
      ORDER BY friendCount DESC
      LIMIT 100
    `);

    const duration = Date.now() - start;
    expect(duration).toBeLessThan(100);
  });
});
```

## 📦 交付物

### 代码模块

- [ ] `src/query/cypher/` - Cypher 查询处理器
- [ ] `src/query/gremlin/` - Gremlin 适配器
- [ ] `src/query/graphql/` - GraphQL 接口
- [ ] `src/adapters/` - 外部标准适配器

### 文档

- [ ] Cypher 语法参考
- [ ] Gremlin 使用指南
- [ ] GraphQL API 文档
- [ ] 迁移指南（从 Neo4j/TinkerGraph）

### 工具

- [ ] Cypher 查询验证器
- [ ] GraphQL Schema 生成器
- [ ] 性能基准对比工具

## ✅ 验收标准

- [ ] Cypher 核心语法 90% 兼容
- [ ] Gremlin 基础遍历 85% 兼容
- [ ] GraphQL 基础查询完全支持
- [ ] 性能指标达标
- [ ] 所有兼容性测试通过

## 🚀 下一步

完成标准兼容后，进入 [Milestone-3] 高级特性阶段，实现全文搜索、图算法等高级功能。
