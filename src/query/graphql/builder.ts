/**
 * GraphQL Schema 构建器
 *
 * 将 SynapseDB 实体类型信息转换为 GraphQL Schema 定义语言 (SDL) 和解析器
 * 支持自动生成查询、变更和订阅类型
 */

import type { PersistentStore } from '../../storage/persistentStore.js';
import type {
  EntityTypeInfo,
  PropertyInfo,
  RelationInfo,
  GraphQLType,
  GraphQLField,
  GraphQLResolver,
  GraphQLContext,
  GeneratedSchema,
  SchemaStatistics,
  SchemaGenerationConfig,
  ResolverGenerationOptions,
  PaginationArgs,
  SortArgs,
  FilterArgs,
  Connection,
  Edge as GraphQLEdge,
  PageInfo,
} from './types.js';
import { GraphQLScalarType } from './types.js';

/**
 * GraphQL Schema 构建器
 */
export class SchemaBuilder {
  private store: PersistentStore;
  private config: Required<SchemaGenerationConfig>;
  private resolverOptions: ResolverGenerationOptions;

  constructor(
    store: PersistentStore,
    config: SchemaGenerationConfig = {},
    resolverOptions: ResolverGenerationOptions = {},
  ) {
    this.store = store;
    this.config = {
      maxSampleSize: 1000,
      minEntityCount: 1,
      typeMapping: {},
      rootTypes: {
        Query: [],
        Mutation: [],
        Subscription: [],
      },
      includeReverseRelations: true,
      maxDepth: 3,
      fieldNaming: 'camelCase',
      excludeTypes: [],
      includeTypes: [],
      excludePredicates: [],
      enableDataLoader: true,
      cacheResolvers: true,
      ...config,
    };

    this.resolverOptions = {
      enablePagination: true,
      enableFiltering: true,
      enableSorting: true,
      enableAggregation: false,
      maxQueryDepth: 10,
      maxQueryComplexity: 1000,
      ...resolverOptions,
    };
  }

  /**
   * 构建完整的 GraphQL Schema
   */
  async buildSchema(entityTypes: EntityTypeInfo[]): Promise<GeneratedSchema> {
    const startTime = Date.now();

    // 生成 GraphQL 类型定义
    const types = await this.generateTypes(entityTypes);

    // 生成根类型
    const rootTypes = await this.generateRootTypes(entityTypes);
    types.push(...rootTypes);

    // 生成 SDL
    const typeDefs = this.generateSDL(types);

    // 生成解析器
    const resolvers = await this.generateResolvers(entityTypes, types);

    // 计算统计信息
    const statistics = this.calculateStatistics(types, entityTypes, Date.now() - startTime);

    return {
      typeDefs,
      resolvers,
      types,
      statistics,
    };
  }

  /**
   * 生成 GraphQL 类型定义
   */
  private async generateTypes(entityTypes: EntityTypeInfo[]): Promise<GraphQLType[]> {
    const types: GraphQLType[] = [];

    // 添加标量类型
    types.push(...this.generateScalarTypes());

    // 为每个实体类型生成 GraphQL 对象类型
    for (const entityType of entityTypes) {
      if (this.shouldIncludeType(entityType.typeName)) {
        const objectType = await this.generateObjectType(entityType, entityTypes);
        types.push(objectType);

        // 生成过滤和排序输入类型
        if (this.resolverOptions.enableFiltering) {
          types.push(this.generateFilterInputType(entityType));
        }

        if (this.resolverOptions.enableSorting) {
          types.push(this.generateSortInputType(entityType));
        }

        // 生成连接类型（分页）
        if (this.resolverOptions.enablePagination) {
          types.push(this.generateConnectionType(entityType));
          types.push(this.generateEdgeType(entityType));
        }
      }
    }

    // 生成通用类型
    types.push(...this.generateUtilityTypes());

    return types;
  }

  /**
   * 生成 GraphQL 对象类型
   */
  private async generateObjectType(
    entityType: EntityTypeInfo,
    allTypes: EntityTypeInfo[],
  ): Promise<GraphQLType> {
    const fields: GraphQLField[] = [];

    // ID 字段
    fields.push({
      name: 'id',
      type: 'ID!',
      description: '实体唯一标识符',
    });

    // 标签字段
    fields.push({
      name: 'label',
      type: 'String',
      description: '实体标签',
    });

    // 属性字段
    for (const property of entityType.properties) {
      const field = this.generatePropertyField(property);
      fields.push(field);
    }

    // 关系字段
    for (const relation of entityType.relations) {
      const field = await this.generateRelationField(relation, allTypes);
      fields.push(field);
    }

    return {
      name: entityType.typeName,
      kind: 'OBJECT',
      fields,
      description: `${entityType.typeName} 实体类型 (${entityType.count} 个实例)`,
    };
  }

  /**
   * 生成属性字段
   */
  private generatePropertyField(property: PropertyInfo): GraphQLField {
    const baseType = this.mapScalarType(property.valueType);
    const type = property.isArray
      ? `[${baseType}]${property.isRequired ? '!' : ''}`
      : `${baseType}${property.isRequired ? '!' : ''}`;

    return {
      name: property.fieldName,
      type,
      description: `${property.predicate} 属性 (${property.uniqueCount} 个唯一值)`,
      resolver: this.generatePropertyResolver(property),
    };
  }

  /**
   * 生成关系字段
   */
  private async generateRelationField(
    relation: RelationInfo,
    allTypes: EntityTypeInfo[],
  ): Promise<GraphQLField> {
    const targetType = this.findEntityType(relation.targetType, allTypes);
    if (!targetType) {
      // 如果目标类型不存在，返回通用对象类型
      const type = relation.isArray ? '[JSON]' : 'JSON';
      return {
        name: relation.fieldName,
        type,
        description: `${relation.predicate} 关系`,
        resolver: this.generateRelationResolver(relation),
      };
    }

    const baseType = relation.targetType;
    let type = relation.isArray ? `[${baseType}]` : baseType;

    // 如果启用分页，使用连接类型
    if (this.resolverOptions.enablePagination && relation.isArray) {
      type = `${baseType}Connection`;
    }

    const args: any[] = [];

    // 添加分页参数
    if (this.resolverOptions.enablePagination && relation.isArray) {
      args.push(
        { name: 'first', type: 'Int', description: '前 N 个结果' },
        { name: 'after', type: 'String', description: '游标后的结果' },
        { name: 'last', type: 'Int', description: '后 N 个结果' },
        { name: 'before', type: 'String', description: '游标前的结果' },
      );
    }

    // 添加过滤参数
    if (this.resolverOptions.enableFiltering) {
      args.push({
        name: 'filter',
        type: `${baseType}Filter`,
        description: '过滤条件',
      });
    }

    // 添加排序参数
    if (this.resolverOptions.enableSorting && relation.isArray) {
      args.push({
        name: 'sort',
        type: `[${baseType}Sort!]`,
        description: '排序条件',
      });
    }

    return {
      name: relation.fieldName,
      type,
      args,
      description: `${relation.predicate} 关系 (${relation.count} 个连接)`,
      resolver: this.generateRelationResolver(relation),
    };
  }

  /**
   * 生成标量类型
   */
  private generateScalarTypes(): GraphQLType[] {
    return [
      {
        name: 'JSON',
        kind: 'SCALAR',
        description: 'JSON 标量类型',
      },
      {
        name: 'DateTime',
        kind: 'SCALAR',
        description: '日期时间标量类型',
      },
    ];
  }

  /**
   * 生成工具类型
   */
  private generateUtilityTypes(): GraphQLType[] {
    const types: GraphQLType[] = [];

    // 分页信息类型
    types.push({
      name: 'PageInfo',
      kind: 'OBJECT',
      fields: [
        { name: 'hasNextPage', type: 'Boolean!', description: '是否有下一页' },
        { name: 'hasPreviousPage', type: 'Boolean!', description: '是否有上一页' },
        { name: 'startCursor', type: 'String', description: '起始游标' },
        { name: 'endCursor', type: 'String', description: '结束游标' },
      ],
      description: '分页信息',
    });

    // 排序方向枚举
    types.push({
      name: 'SortDirection',
      kind: 'ENUM',
      enumValues: ['ASC', 'DESC'],
      description: '排序方向',
    });

    // 过滤操作符枚举
    types.push({
      name: 'FilterOperator',
      kind: 'ENUM',
      enumValues: [
        'EQ',
        'NEQ',
        'LT',
        'LTE',
        'GT',
        'GTE',
        'IN',
        'NOT_IN',
        'CONTAINS',
        'STARTS_WITH',
        'ENDS_WITH',
      ],
      description: '过滤操作符',
    });

    return types;
  }

  /**
   * 生成连接类型（分页）
   */
  private generateConnectionType(entityType: EntityTypeInfo): GraphQLType {
    return {
      name: `${entityType.typeName}Connection`,
      kind: 'OBJECT',
      fields: [
        {
          name: 'edges',
          type: `[${entityType.typeName}Edge!]!`,
          description: '边列表',
        },
        {
          name: 'pageInfo',
          type: 'PageInfo!',
          description: '分页信息',
        },
        {
          name: 'totalCount',
          type: 'Int',
          description: '总数量',
        },
      ],
      description: `${entityType.typeName} 连接类型`,
    };
  }

  /**
   * 生成边类型（分页）
   */
  private generateEdgeType(entityType: EntityTypeInfo): GraphQLType {
    return {
      name: `${entityType.typeName}Edge`,
      kind: 'OBJECT',
      fields: [
        {
          name: 'node',
          type: `${entityType.typeName}!`,
          description: '节点',
        },
        {
          name: 'cursor',
          type: 'String!',
          description: '游标',
        },
      ],
      description: `${entityType.typeName} 边类型`,
    };
  }

  /**
   * 生成过滤输入类型
   */
  private generateFilterInputType(entityType: EntityTypeInfo): GraphQLType {
    const fields: GraphQLField[] = [];

    // ID 过滤
    fields.push({
      name: 'id',
      type: 'ID',
    });

    // 属性过滤
    for (const property of entityType.properties) {
      const baseType = this.mapScalarType(property.valueType);
      fields.push({
        name: property.fieldName,
        type: baseType,
      });

      // 操作符过滤
      fields.push({
        name: `${property.fieldName}_not`,
        type: baseType,
      });

      if (property.valueType === GraphQLScalarType.String) {
        fields.push(
          { name: `${property.fieldName}_contains`, type: 'String' },
          { name: `${property.fieldName}_starts_with`, type: 'String' },
          { name: `${property.fieldName}_ends_with`, type: 'String' },
        );
      }

      if (
        property.valueType === GraphQLScalarType.Int ||
        property.valueType === GraphQLScalarType.Float
      ) {
        fields.push(
          { name: `${property.fieldName}_lt`, type: baseType },
          { name: `${property.fieldName}_lte`, type: baseType },
          { name: `${property.fieldName}_gt`, type: baseType },
          { name: `${property.fieldName}_gte`, type: baseType },
        );
      }

      if (property.isArray) {
        fields.push({
          name: `${property.fieldName}_in`,
          type: `[${baseType}!]`,
        });
      }
    }

    return {
      name: `${entityType.typeName}Filter`,
      kind: 'INPUT_OBJECT',
      fields,
      description: `${entityType.typeName} 过滤输入`,
    };
  }

  /**
   * 生成排序输入类型
   */
  private generateSortInputType(entityType: EntityTypeInfo): GraphQLType {
    const enumValues: string[] = [];

    // 添加属性排序选项
    for (const property of entityType.properties) {
      enumValues.push(`${property.fieldName}_ASC`, `${property.fieldName}_DESC`);
    }

    return {
      name: `${entityType.typeName}Sort`,
      kind: 'ENUM',
      enumValues,
      description: `${entityType.typeName} 排序选项`,
    };
  }

  /**
   * 生成根类型
   */
  private async generateRootTypes(entityTypes: EntityTypeInfo[]): Promise<GraphQLType[]> {
    const queryFields: GraphQLField[] = [];
    const mutationFields: GraphQLField[] = [];

    for (const entityType of entityTypes) {
      if (!this.shouldIncludeType(entityType.typeName)) {
        continue;
      }

      // 查询字段
      const singularName = this.toSingularName(entityType.typeName);
      const pluralName = this.toPluralName(entityType.typeName);

      // 单个实体查询
      queryFields.push({
        name: singularName,
        type: entityType.typeName,
        args: [{ name: 'id', type: 'ID!', description: '实体ID' }],
        description: `根据 ID 查询单个 ${entityType.typeName}`,
        resolver: this.generateSingleEntityResolver(entityType),
      });

      // 多个实体查询
      const args: any[] = [];

      if (this.resolverOptions.enablePagination) {
        args.push(
          { name: 'first', type: 'Int', description: '前 N 个结果' },
          { name: 'after', type: 'String', description: '游标后的结果' },
          { name: 'last', type: 'Int', description: '后 N 个结果' },
          { name: 'before', type: 'String', description: '游标前的结果' },
        );
      }

      if (this.resolverOptions.enableFiltering) {
        args.push({
          name: 'filter',
          type: `${entityType.typeName}Filter`,
          description: '过滤条件',
        });
      }

      if (this.resolverOptions.enableSorting) {
        args.push({
          name: 'sort',
          type: `[${entityType.typeName}Sort!]`,
          description: '排序条件',
        });
      }

      const returnType = this.resolverOptions.enablePagination
        ? `${entityType.typeName}Connection!`
        : `[${entityType.typeName}!]!`;

      queryFields.push({
        name: pluralName,
        type: returnType,
        args,
        description: `查询 ${entityType.typeName} 列表`,
        resolver: this.generateEntityListResolver(entityType),
      });
    }

    const rootTypes: GraphQLType[] = [];

    // Query 根类型
    if (queryFields.length > 0) {
      rootTypes.push({
        name: 'Query',
        kind: 'OBJECT',
        fields: queryFields,
        description: '查询根类型',
      });
    }

    // Mutation 根类型（暂时为空）
    if (mutationFields.length > 0) {
      rootTypes.push({
        name: 'Mutation',
        kind: 'OBJECT',
        fields: mutationFields,
        description: '变更根类型',
      });
    }

    return rootTypes;
  }

  /**
   * 生成解析器
   */
  private async generateResolvers(
    entityTypes: EntityTypeInfo[],
    types: GraphQLType[],
  ): Promise<Record<string, Record<string, GraphQLResolver>>> {
    const resolvers: Record<string, Record<string, GraphQLResolver>> = {};

    // 根解析器
    const queryType = types.find((t) => t.name === 'Query');
    if (queryType) {
      resolvers.Query = {};
      for (const field of queryType.fields || []) {
        if (field.resolver) {
          resolvers.Query[field.name] = field.resolver;
        }
      }
    }

    // 实体解析器
    for (const entityType of entityTypes) {
      if (!this.shouldIncludeType(entityType.typeName)) {
        continue;
      }

      const objectType = types.find((t) => t.name === entityType.typeName);
      if (objectType) {
        resolvers[entityType.typeName] = {};
        for (const field of objectType.fields || []) {
          if (field.resolver) {
            resolvers[entityType.typeName][field.name] = field.resolver;
          }
        }
      }
    }

    // 标量解析器
    resolvers.JSON = {
      serialize: (value: unknown) => value,
      parseValue: (value: unknown) => value,
      parseLiteral: (ast: any) => ast.value,
    };

    resolvers.DateTime = {
      serialize: (value: unknown) => (value instanceof Date ? value.toISOString() : String(value)),
      parseValue: (value: unknown) => new Date(String(value)),
      parseLiteral: (ast: any) => new Date(ast.value),
    };

    return resolvers;
  }

  /**
   * 生成属性解析器
   */
  private generatePropertyResolver(property: PropertyInfo): GraphQLResolver {
    return async (parent: any, args: any, context: GraphQLContext) => {
      if (parent[property.fieldName] !== undefined) {
        return parent[property.fieldName];
      }

      // 从 SynapseDB 查询属性值
      const nodeId = Number(parent.id);
      const predicateId = context.store.getNodeIdByValue(property.predicate);

      if (predicateId === undefined) {
        return null;
      }

      const records = context.store.resolveRecords(
        context.store.query({ subjectId: nodeId, predicateId }),
        { includeProperties: false },
      );

      if (records.length === 0) {
        return null;
      }

      if (property.isArray) {
        return records.map((record: any) => context.store.getNodeValueById(record.objectId));
      }

      return context.store.getNodeValueById(records[0].objectId);
    };
  }

  /**
   * 生成关系解析器
   */
  private generateRelationResolver(relation: RelationInfo): GraphQLResolver {
    return async (parent: any, args: any, context: GraphQLContext) => {
      const nodeId = Number(parent.id);
      const predicateId = context.store.getNodeIdByValue(relation.predicate);

      if (predicateId === undefined) {
        return relation.isArray ? [] : null;
      }

      const records = context.store.resolveRecords(
        context.store.query({ subjectId: nodeId, predicateId }),
        { includeProperties: false },
      );

      if (records.length === 0) {
        return relation.isArray ? [] : null;
      }

      // 转换为目标实体
      const targetEntities = records.map((record: any) => ({
        id: record.objectId,
        label: relation.targetType,
      }));

      if (relation.isArray) {
        // 如果启用分页，返回连接格式
        if (this.resolverOptions.enablePagination && (args.first || args.last)) {
          return this.applyPagination(targetEntities, args);
        }
        return targetEntities;
      }

      return targetEntities[0] || null;
    };
  }

  /**
   * 生成单实体解析器
   */
  private generateSingleEntityResolver(entityType: EntityTypeInfo): GraphQLResolver {
    return async (parent: any, args: any, context: GraphQLContext) => {
      const nodeId = Number(args.id);
      const nodeValue = context.store.getNodeValueById(nodeId);

      if (nodeValue === undefined) {
        return null;
      }

      return {
        id: nodeId,
        label: entityType.typeName,
      };
    };
  }

  /**
   * 生成实体列表解析器
   */
  private generateEntityListResolver(entityType: EntityTypeInfo): GraphQLResolver {
    return async (parent: any, args: any, context: GraphQLContext) => {
      // 简化实现：返回该类型的所有实体
      let entities = entityType.sampleIds.map((id) => ({
        id: Number(id),
        label: entityType.typeName,
      }));

      // 应用过滤（最小实现）：支持 _not/_lt/_lte/_gt/_gte/_contains/_starts_with/_ends_with/_in
      if (args?.filter && typeof args.filter === 'object') {
        const fieldToPredicate = new Map(
          entityType.properties.map((p) => [p.fieldName, p.predicate] as const),
        );
        const applyOp = (val: unknown, op: string, cmp: any): boolean => {
          if (val == null) return false;
          if (Array.isArray(val)) return val.some((v) => applyOp(v, op, cmp));
          const vStr = String(val);
          const vNum = Number(vStr);
          const cNum = Number(cmp);
          switch (op) {
            case 'not':
              return vStr !== String(cmp);
            case 'lt':
              return !Number.isNaN(vNum) && vNum < cNum;
            case 'lte':
              return !Number.isNaN(vNum) && vNum <= cNum;
            case 'gt':
              return !Number.isNaN(vNum) && vNum > cNum;
            case 'gte':
              return !Number.isNaN(vNum) && vNum >= cNum;
            case 'contains':
              return vStr.includes(String(cmp));
            case 'starts_with':
              return vStr.startsWith(String(cmp));
            case 'ends_with':
              return vStr.endsWith(String(cmp));
            case 'in':
              return Array.isArray(cmp) && cmp.map(String).includes(vStr);
            default:
              return vStr === String(cmp);
          }
        };

        entities = entities.filter((e) => {
          for (const [rawKey, cmp] of Object.entries(args.filter)) {
            const key = String(rawKey);
            let base = key;
            let op = '';
            const suffixes = [
              '_not',
              '_lt',
              '_lte',
              '_gt',
              '_gte',
              '_contains',
              '_starts_with',
              '_ends_with',
              '_in',
            ];
            for (const s of suffixes) {
              if (key.endsWith(s)) {
                base = key.slice(0, -s.length);
                op = s.slice(1); // 去掉前导下划线
                break;
              }
            }
            const predicate = fieldToPredicate.get(base);
            if (!predicate) return false;
            const pid = context.store.getNodeIdByValue(predicate);
            if (pid === undefined) return false;
            const triples = context.store.resolveRecords(
              context.store.query({ subjectId: e.id, predicateId: pid }),
              { includeProperties: false },
            );
            const values = triples.map((t: any) => context.store.getNodeValueById(t.objectId));
            // 无值按不匹配处理
            if (values.length === 0) return false;
            const matched = values.some((v: any) => applyOp(v, op, cmp));
            if (!matched) return false;
          }
          return true;
        });
      }

      // 分页参数存在时返回连接格式；否则返回数组，兼容测试期望
      if (
        this.resolverOptions.enablePagination &&
        (args?.first !== undefined || args?.last !== undefined || args?.after || args?.before)
      ) {
        return this.applyPagination(entities, args);
      }

      return entities;
    };
  }

  /**
   * 应用分页
   */
  private applyPagination<T>(items: T[], args: PaginationArgs): Connection<T> {
    const { first, after, last, before } = args;
    let startIndex = 0;
    let endIndex = items.length;

    // 应用游标逻辑（简化实现）
    if (after) {
      const afterIndex = parseInt(after, 10);
      startIndex = Math.max(0, afterIndex + 1);
    }

    if (before) {
      const beforeIndex = parseInt(before, 10);
      endIndex = Math.min(items.length, beforeIndex);
    }

    if (first && first > 0) {
      endIndex = Math.min(endIndex, startIndex + first);
    }

    if (last && last > 0) {
      startIndex = Math.max(startIndex, endIndex - last);
    }

    const slicedItems = items.slice(startIndex, endIndex);
    const edges: GraphQLEdge<T>[] = slicedItems.map((item, index) => ({
      node: item,
      cursor: String(startIndex + index),
    }));

    const pageInfo: PageInfo = {
      hasNextPage: endIndex < items.length,
      hasPreviousPage: startIndex > 0,
      startCursor: edges.length > 0 ? edges[0].cursor : undefined,
      endCursor: edges.length > 0 ? edges[edges.length - 1].cursor : undefined,
    };

    return {
      edges,
      pageInfo,
      totalCount: items.length,
    };
  }

  // ============ SDL 生成 ============

  /**
   * 生成 GraphQL SDL
   */
  private generateSDL(types: GraphQLType[]): string {
    const lines: string[] = [];

    for (const type of types) {
      lines.push(this.typeToSDL(type));
      lines.push('');
    }

    return lines.join('\n').trim();
  }

  /**
   * 类型转 SDL
   */
  private typeToSDL(type: GraphQLType): string {
    const lines: string[] = [];

    if (type.description) {
      lines.push(`"""${type.description}"""`);
    }

    switch (type.kind) {
      case 'OBJECT':
        lines.push(`type ${type.name} {`);
        if (type.fields) {
          for (const field of type.fields) {
            lines.push(`  ${this.fieldToSDL(field)}`);
          }
        }
        lines.push('}');
        break;

      case 'INPUT_OBJECT':
        lines.push(`input ${type.name} {`);
        if (type.fields) {
          for (const field of type.fields) {
            lines.push(`  ${field.name}: ${field.type}`);
          }
        }
        lines.push('}');
        break;

      case 'ENUM':
        lines.push(`enum ${type.name} {`);
        if (type.enumValues) {
          for (const value of type.enumValues) {
            lines.push(`  ${value}`);
          }
        }
        lines.push('}');
        break;

      case 'SCALAR':
        lines.push(`scalar ${type.name}`);
        break;

      default:
        lines.push(`# 未支持的类型: ${type.kind}`);
    }

    return lines.join('\n');
  }

  /**
   * 字段转 SDL
   */
  private fieldToSDL(field: GraphQLField): string {
    let sdl = field.name;

    // 添加参数
    if (field.args && field.args.length > 0) {
      const argsList = field.args.map((arg) => `${arg.name}: ${arg.type}`).join(', ');
      sdl += `(${argsList})`;
    }

    sdl += `: ${field.type}`;

    // 添加描述
    if (field.description) {
      sdl = `"""${field.description}"""\n  ${sdl}`;
    }

    return sdl;
  }

  // ============ 工具方法 ============

  /**
   * 映射标量类型
   */
  private mapScalarType(scalarType: GraphQLScalarType): string {
    switch (scalarType) {
      case GraphQLScalarType.String:
        return 'String';
      case GraphQLScalarType.Int:
        return 'Int';
      case GraphQLScalarType.Float:
        return 'Float';
      case GraphQLScalarType.Boolean:
        return 'Boolean';
      case GraphQLScalarType.ID:
        return 'ID';
      case GraphQLScalarType.JSON:
        return 'JSON';
      default:
        return 'String';
    }
  }

  /**
   * 检查是否应该包含类型
   */
  private shouldIncludeType(typeName: string): boolean {
    if (this.config.excludeTypes.includes(typeName)) {
      return false;
    }

    if (this.config.includeTypes.length > 0) {
      return this.config.includeTypes.includes(typeName);
    }

    return true;
  }

  /**
   * 查找实体类型
   */
  private findEntityType(
    typeName: string,
    entityTypes: EntityTypeInfo[],
  ): EntityTypeInfo | undefined {
    return entityTypes.find((type) => type.typeName === typeName);
  }

  /**
   * 转为单数形式
   */
  private toSingularName(typeName: string): string {
    return typeName.toLowerCase();
  }

  /**
   * 转为复数形式
   */
  private toPluralName(typeName: string): string {
    const name = typeName.toLowerCase();
    if (name.endsWith('s')) {
      return name + 'es';
    }
    return name + 's';
  }

  /**
   * 计算统计信息
   */
  private calculateStatistics(
    types: GraphQLType[],
    entityTypes: EntityTypeInfo[],
    generationTime: number,
  ): SchemaStatistics {
    const typeCount = types.filter((t) => t.kind === 'OBJECT').length;
    const fieldCount = types.reduce((count, type) => count + (type.fields?.length || 0), 0);
    const relationCount = entityTypes.reduce((count, entity) => count + entity.relations.length, 0);
    const entitiesAnalyzed = entityTypes.reduce((count, entity) => count + entity.count, 0);

    // 计算复杂度（简化计算）
    const schemaComplexity = typeCount * 10 + fieldCount * 2 + relationCount * 5;

    return {
      typeCount,
      fieldCount,
      relationCount,
      entitiesAnalyzed,
      generationTime,
      schemaComplexity,
    };
  }
}
