/**
 * GraphQL Schema 发现和分析器
 *
 * 从 SynapseDB 知识图谱中分析和推断 GraphQL 类型结构
 * 支持实体类型发现、属性分析和关系映射
 */

import type { PersistentStore } from '../../storage/persistentStore.js';
import type {
  EntityTypeInfo,
  PropertyInfo,
  RelationInfo,
  SchemaGenerationConfig,
} from './types.js';
import { GraphQLScalarType } from './types.js';

/**
 * Schema 发现器
 *
 * 负责从 SynapseDB 数据中发现和分析图谱结构
 */
export class SchemaDiscovery {
  private store: PersistentStore;
  private config: Required<SchemaGenerationConfig>;

  constructor(store: PersistentStore, config: SchemaGenerationConfig = {}) {
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
  }

  /**
   * 发现所有实体类型
   */
  async discoverEntityTypes(): Promise<EntityTypeInfo[]> {
    const entityTypes = new Map<string, EntityTypeInfo>();

    // 分析所有记录以发现类型模式
    const records = this.store.resolveRecords(this.store.query({}), { includeProperties: false });

    // 收集类型信息
    const typePredicates = new Set(['TYPE', 'type', 'rdf:type', 'a']);
    const entityTypeMap = new Map<number, string>();

    // 第一轮：查找明确的类型声明
    for (const record of records) {
      const predicateValue = this.store.getNodeValueById(record.predicateId);
      const objectValue = this.store.getNodeValueById(record.objectId);

      if (predicateValue && typePredicates.has(String(predicateValue))) {
        const typeName = String(objectValue);
        if (this.shouldIncludeType(typeName)) {
          entityTypeMap.set(record.subjectId, typeName);
        }
      }
    }

    // 第二轮：基于结构模式推断类型
    const structuralTypes = await this.inferStructuralTypes(records, entityTypeMap);

    // 合并类型信息
    for (const [nodeId, typeName] of [...entityTypeMap.entries(), ...structuralTypes.entries()]) {
      if (!entityTypes.has(typeName)) {
        entityTypes.set(typeName, {
          typeName,
          count: 0,
          sampleIds: [],
          properties: [],
          relations: [],
        });
      }

      const typeInfo = entityTypes.get(typeName)!;
      typeInfo.count++;

      if (typeInfo.sampleIds.length < this.config.maxSampleSize) {
        typeInfo.sampleIds.push(nodeId);
      }
    }

    // 分析每个类型的属性和关系
    for (const typeInfo of entityTypes.values()) {
      if (typeInfo.count >= this.config.minEntityCount) {
        await this.analyzeTypeProperties(typeInfo, entityTypeMap);
        await this.analyzeTypeRelations(typeInfo, entityTypeMap);
      }
    }

    return Array.from(entityTypes.values())
      .filter((type) => type.count >= this.config.minEntityCount)
      .sort((a, b) => b.count - a.count);
  }

  /**
   * 基于结构模式推断类型
   */
  private async inferStructuralTypes(
    records: any[],
    existingTypes: Map<number, string>,
  ): Promise<Map<number, string>> {
    const structuralTypes = new Map<number, string>();
    const nodeSignatures = new Map<number, Set<string>>();

    // 收集每个节点的谓词签名
    for (const record of records) {
      const predicateValue = this.store.getNodeValueById(record.predicateId);
      if (!predicateValue || this.shouldExcludePredicate(String(predicateValue))) {
        continue;
      }

      if (!nodeSignatures.has(record.subjectId)) {
        nodeSignatures.set(record.subjectId, new Set());
      }
      nodeSignatures.get(record.subjectId)!.add(String(predicateValue));
    }

    // 按签名聚类
    const signatureClusters = new Map<string, number[]>();

    for (const [nodeId, signature] of nodeSignatures.entries()) {
      if (existingTypes.has(nodeId)) {
        continue; // 跳过已知类型的节点
      }

      const signatureKey = Array.from(signature).sort().join('|');
      if (!signatureClusters.has(signatureKey)) {
        signatureClusters.set(signatureKey, []);
      }
      signatureClusters.get(signatureKey)!.push(nodeId);
    }

    // 为大型聚类生成类型名
    let clusterId = 0;
    for (const [signatureKey, nodes] of signatureClusters.entries()) {
      if (nodes.length >= this.config.minEntityCount) {
        const typeName = this.generateStructuralTypeName(signatureKey, clusterId++);
        for (const nodeId of nodes) {
          structuralTypes.set(nodeId, typeName);
        }
      }
    }

    return structuralTypes;
  }

  /**
   * 生成结构化类型名称
   */
  private generateStructuralTypeName(signatureKey: string, clusterId: number): string {
    const predicates = signatureKey.split('|');

    // 尝试从主要谓词推断类型名
    const commonEntityPredicates = ['HAS_NAME', 'name', 'title', 'label'];
    const namePredicates = predicates.filter((p) =>
      commonEntityPredicates.some((common) => p.toLowerCase().includes(common.toLowerCase())),
    );

    if (namePredicates.length > 0) {
      const baseName = namePredicates[0].replace(/^(HAS_|has_)/i, '');
      return this.toPascalCase(baseName) + 'Entity';
    }

    // 使用最常见的谓词作为基础
    const basePredicate = predicates[0];
    return this.toPascalCase(basePredicate.replace(/^(HAS_|has_)/i, '')) + 'Type';
  }

  /**
   * 分析类型属性
   */
  private async analyzeTypeProperties(
    typeInfo: EntityTypeInfo,
    entityTypeMap: Map<number, string>,
  ): Promise<void> {
    const propertyMap = new Map<
      string,
      {
        values: unknown[];
        requiredCount: number;
        arrayCount: number;
      }
    >();

    // 分析样本节点的属性
    for (const nodeId of typeInfo.sampleIds.slice(0, this.config.maxSampleSize)) {
      const records = this.store.resolveRecords(this.store.query({ subjectId: Number(nodeId) }), {
        includeProperties: false,
      });

      for (const record of records) {
        const predicateValue = this.store.getNodeValueById(record.predicateId);
        const objectValue = this.store.getNodeValueById(record.objectId);

        if (!predicateValue || this.shouldExcludePredicate(String(predicateValue))) {
          continue;
        }

        // 跳过指向其他实体的关系
        if (entityTypeMap.has(record.objectId)) {
          continue;
        }

        const predicate = String(predicateValue);
        if (!propertyMap.has(predicate)) {
          propertyMap.set(predicate, {
            values: [],
            requiredCount: 0,
            arrayCount: 0,
          });
        }

        const propInfo = propertyMap.get(predicate)!;
        propInfo.values.push(objectValue);
        propInfo.requiredCount++;
      }
    }

    // 转换为 PropertyInfo 数组
    typeInfo.properties = Array.from(propertyMap.entries())
      .map(([predicate, info]) => {
        const fieldName = this.convertFieldName(predicate);
        const valueType = this.inferValueType(info.values);
        const isRequired = info.requiredCount / typeInfo.sampleIds.length > 0.8; // 80% 的实体都有此属性
        const isArray = info.arrayCount > 0;

        return {
          predicate,
          fieldName,
          valueType,
          isRequired,
          isArray,
          uniqueCount: new Set(info.values).size,
          samples: info.values.slice(0, 10), // 保留前10个样本值
        };
      })
      .sort((a, b) => b.uniqueCount - a.uniqueCount);
  }

  /**
   * 分析类型关系
   */
  private async analyzeTypeRelations(
    typeInfo: EntityTypeInfo,
    entityTypeMap: Map<number, string>,
  ): Promise<void> {
    const relationMap = new Map<
      string,
      {
        targetTypes: Map<string, number>;
        count: number;
      }
    >();

    // 分析外向关系
    for (const nodeId of typeInfo.sampleIds.slice(0, this.config.maxSampleSize)) {
      const records = this.store.resolveRecords(this.store.query({ subjectId: Number(nodeId) }), {
        includeProperties: false,
      });

      for (const record of records) {
        const predicateValue = this.store.getNodeValueById(record.predicateId);

        if (!predicateValue || this.shouldExcludePredicate(String(predicateValue))) {
          continue;
        }

        const targetType = entityTypeMap.get(record.objectId);
        if (!targetType) {
          continue; // 不是实体关系
        }

        const predicate = String(predicateValue);
        if (!relationMap.has(predicate)) {
          relationMap.set(predicate, {
            targetTypes: new Map(),
            count: 0,
          });
        }

        const relInfo = relationMap.get(predicate)!;
        relInfo.count++;
        relInfo.targetTypes.set(targetType, (relInfo.targetTypes.get(targetType) || 0) + 1);
      }
    }

    // 分析内向关系（如果启用）
    if (this.config.includeReverseRelations) {
      await this.analyzeReverseRelations(typeInfo, entityTypeMap, relationMap);
    }

    // 转换为 RelationInfo 数组
    typeInfo.relations = Array.from(relationMap.entries())
      .map(([predicate, info]) => {
        const fieldName = this.convertFieldName(predicate);
        const targetTypes = Array.from(info.targetTypes.entries()).sort((a, b) => b[1] - a[1]); // 按频率排序
        const primaryTargetType = targetTypes[0][0];
        const isArray =
          targetTypes.reduce((sum, [, count]) => sum + count, 0) > typeInfo.sampleIds.length;

        return {
          predicate,
          fieldName,
          targetType: primaryTargetType,
          isArray,
          count: info.count,
        };
      })
      .sort((a, b) => b.count - a.count);
  }

  /**
   * 分析反向关系
   */
  private async analyzeReverseRelations(
    typeInfo: EntityTypeInfo,
    entityTypeMap: Map<number, string>,
    relationMap: Map<string, { targetTypes: Map<string, number>; count: number }>,
  ): Promise<void> {
    for (const nodeId of typeInfo.sampleIds.slice(0, this.config.maxSampleSize)) {
      const records = this.store.resolveRecords(this.store.query({ objectId: Number(nodeId) }), {
        includeProperties: false,
      });

      for (const record of records) {
        const predicateValue = this.store.getNodeValueById(record.predicateId);

        if (!predicateValue || this.shouldExcludePredicate(String(predicateValue))) {
          continue;
        }

        const sourceType = entityTypeMap.get(record.subjectId);
        if (!sourceType) {
          continue;
        }

        const reversePredicate = `${predicateValue}_reverse`;
        if (!relationMap.has(reversePredicate)) {
          relationMap.set(reversePredicate, {
            targetTypes: new Map(),
            count: 0,
          });
        }

        const relInfo = relationMap.get(reversePredicate)!;
        relInfo.count++;
        relInfo.targetTypes.set(sourceType, (relInfo.targetTypes.get(sourceType) || 0) + 1);
      }
    }
  }

  /**
   * 推断值类型
   */
  private inferValueType(values: unknown[]): GraphQLScalarType {
    if (values.length === 0) {
      return GraphQLScalarType.String;
    }

    const typeStats = {
      string: 0,
      number: 0,
      boolean: 0,
      object: 0,
    };

    for (const value of values) {
      if (typeof value === 'string') {
        typeStats.string++;
      } else if (typeof value === 'number') {
        typeStats.number++;
      } else if (typeof value === 'boolean') {
        typeStats.boolean++;
      } else {
        typeStats.object++;
      }
    }

    // 选择最常见的类型
    const maxType = Object.entries(typeStats).reduce((a, b) => (a[1] > b[1] ? a : b))[0];

    switch (maxType) {
      case 'number':
        return values.some((v) => typeof v === 'number' && v % 1 !== 0)
          ? GraphQLScalarType.Float
          : GraphQLScalarType.Int;
      case 'boolean':
        return GraphQLScalarType.Boolean;
      case 'object':
        return GraphQLScalarType.JSON;
      default:
        return GraphQLScalarType.String;
    }
  }

  /**
   * 转换字段名称
   */
  private convertFieldName(predicate: string): string {
    let fieldName = predicate;

    // 移除常见前缀
    fieldName = fieldName.replace(/^(HAS_|has_|IS_|is_)/i, '');

    switch (this.config.fieldNaming) {
      case 'camelCase':
        return this.toCamelCase(fieldName);
      case 'snake_case':
        return this.toSnakeCase(fieldName);
      default:
        return fieldName;
    }
  }

  /**
   * 转换为 camelCase
   */
  private toCamelCase(str: string): string {
    return str
      .toLowerCase()
      .replace(/[_\-\s]+(.)/g, (_, char) => char.toUpperCase())
      .replace(/^./, (char) => char.toLowerCase());
  }

  /**
   * 转换为 PascalCase
   */
  private toPascalCase(str: string): string {
    const camelCase = this.toCamelCase(str);
    return camelCase.charAt(0).toUpperCase() + camelCase.slice(1);
  }

  /**
   * 转换为 snake_case
   */
  private toSnakeCase(str: string): string {
    return str
      .replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
      .replace(/^_/, '')
      .toLowerCase();
  }

  /**
   * 检查是否应该包含此类型
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
   * 检查是否应该排除此谓词
   */
  private shouldExcludePredicate(predicate: string): boolean {
    return this.config.excludePredicates.includes(predicate);
  }
}
