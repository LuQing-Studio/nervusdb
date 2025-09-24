/**
 * Gremlin 查询执行引擎
 *
 * 将 Gremlin 步骤序列转换为 SynapseDB 查询并执行
 * 支持流式处理和优化执行
 */

import type { PersistentStore } from '../../storage/persistentStore.js';
import type { Vertex, Edge, Direction, Predicate, ElementId, PropertyValue } from './types.js';
import { P, GremlinError, TraversalError } from './types.js';
import type { GremlinStep } from './step.js';
import type { TraversalResult } from './traversal.js';

// 执行上下文
interface ExecutionContext {
  currentElements: Set<Vertex | Edge>;
  labeledElements: Map<string, Set<Vertex | Edge>>;
  pathHistory: Map<string, (Vertex | Edge)[]>;
  stepIndex: number;
  bulkMap: Map<string, number>;
}

// 元素适配器 - 将 SynapseDB 数据转换为 Gremlin 元素格式
class ElementAdapter {
  private store: PersistentStore;

  constructor(store: PersistentStore) {
    this.store = store;
  }

  /**
   * 将节点ID转换为Vertex
   */
  nodeToVertex(nodeId: number): Vertex {
    const value = this.store.getNodeValueById(nodeId);
    const properties = this.store.getNodeProperties(nodeId) || {};
    const labels = this.extractLabels(properties);

    // 加载所有以该节点为主体的边作为属性
    const allProperties = { ...properties };
    const records = this.store.resolveRecords(this.store.query({ subjectId: nodeId }), {
      includeProperties: false,
    });

    for (const record of records) {
      const predicateValue = this.store.getNodeValueById(record.predicateId);
      const objectValue = this.store.getNodeValueById(record.objectId);

      if (predicateValue && objectValue !== undefined) {
        const key = String(predicateValue);
        if (!allProperties[key]) {
          allProperties[key] = objectValue;
        }
      }
    }

    return {
      type: 'vertex',
      id: nodeId,
      label: labels.length > 0 ? labels[0] : 'vertex',
      properties: { ...allProperties, __labels: labels },
    };
  }

  /**
   * 将三元组转换为Edge
   */
  tripleToEdge(subjectId: number, predicateId: number, objectId: number): Edge {
    const predicateValue = this.store.getNodeValueById(predicateId) || 'edge';
    const properties = {}; // 简化处理，边属性暂时为空

    return {
      type: 'edge',
      id: `${subjectId}-${predicateId}-${objectId}`,
      label: String(predicateValue),
      inVertex: objectId,
      outVertex: subjectId,
      properties,
    };
  }

  /**
   * 从属性中提取标签
   */
  private extractLabels(properties: Record<string, unknown>): string[] {
    if (properties.__labels && Array.isArray(properties.__labels)) {
      return properties.__labels as string[];
    }
    if (properties.label) {
      return [String(properties.label)];
    }
    if (properties.type) {
      return [String(properties.type)];
    }
    return [];
  }
}

/**
 * Gremlin 执行引擎
 */
export class GremlinExecutor {
  private store: PersistentStore;
  private adapter: ElementAdapter;

  constructor(store: PersistentStore) {
    this.store = store;
    this.adapter = new ElementAdapter(store);
  }

  /**
   * 执行 Gremlin 步骤序列
   */
  async execute<T = Vertex | Edge>(steps: GremlinStep[]): Promise<TraversalResult<T>[]> {
    if (steps.length === 0) {
      return [];
    }

    let context: ExecutionContext = {
      currentElements: new Set(),
      labeledElements: new Map(),
      pathHistory: new Map(),
      stepIndex: 0,
      bulkMap: new Map(),
    };

    // 逐步执行
    for (const step of steps) {
      context = await this.executeStep(step, context);
      context.stepIndex++;
    }

    // 转换为结果格式
    return Array.from(context.currentElements).map((element) => ({
      value: element as T,
      bulk: context.bulkMap.get(this.getElementKey(element)) || 1,
    }));
  }

  /**
   * 执行单个步骤
   */
  private async executeStep(
    step: GremlinStep,
    context: ExecutionContext,
  ): Promise<ExecutionContext> {
    switch (step.type) {
      case 'V':
        return await this.executeV(step, context);
      case 'E':
        return await this.executeE(step, context);
      case 'out':
        return await this.executeOut(step, context);
      case 'in':
        return await this.executeIn(step, context);
      case 'both':
        return await this.executeBoth(step, context);
      case 'outE':
        return await this.executeOutE(step, context);
      case 'inE':
        return await this.executeInE(step, context);
      case 'bothE':
        return await this.executeBothE(step, context);
      case 'inV':
        return await this.executeInV(step, context);
      case 'outV':
        return await this.executeOutV(step, context);
      case 'bothV':
        return await this.executeBothV(step, context);
      case 'has':
        return await this.executeHas(step, context);
      case 'hasLabel':
        return await this.executeHasLabel(step, context);
      case 'hasId':
        return await this.executeHasId(step, context);
      case 'is':
        return await this.executeIs(step, context);
      case 'limit':
        return await this.executeLimit(step, context);
      case 'range':
        return await this.executeRange(step, context);
      case 'skip':
        return await this.executeSkip(step, context);
      case 'dedup':
        return await this.executeDedup(step, context);
      case 'as':
        return await this.executeAs(step, context);
      case 'select':
        return await this.executeSelect(step, context);
      case 'values':
        return await this.executeValues(step, context);
      case 'valueMap':
        return await this.executeValueMap(step, context);
      case 'elementMap':
        return await this.executeElementMap(step, context);
      case 'count':
        return await this.executeCount(step, context);
      case 'fold':
        return await this.executeFold(step, context);
      default:
        throw new TraversalError(`不支持的步骤类型: ${step.type}`);
    }
  }

  // ============ 起始步骤执行 ============

  /**
   * 执行 V() 步骤
   */
  private async executeV(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    if (step.ids && step.ids.length > 0) {
      // 获取指定ID的顶点
      for (const id of step.ids) {
        const nodeId = Number(id);
        // 检查节点是否存在
        const nodeValue = this.store.getNodeValueById(nodeId);
        if (nodeValue !== undefined) {
          try {
            const vertex = this.adapter.nodeToVertex(nodeId);
            newContext.currentElements.add(vertex);
          } catch {
            // 忽略不存在的顶点
          }
        }
      }
    } else {
      // 获取所有顶点
      const allRecords = this.store.resolveRecords(this.store.query({}), {
        includeProperties: false,
      });
      const uniqueNodes = new Set(allRecords.map((r) => r.subjectId));

      for (const nodeId of uniqueNodes) {
        try {
          const vertex = this.adapter.nodeToVertex(nodeId);
          newContext.currentElements.add(vertex);
        } catch {
          // 忽略无效节点
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 E() 步骤
   */
  private async executeE(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    // 获取所有边
    const allRecords = this.store.resolveRecords(this.store.query({}), {
      includeProperties: false,
    });

    for (const record of allRecords) {
      try {
        const edge = this.adapter.tripleToEdge(
          record.subjectId,
          record.predicateId,
          record.objectId,
        );
        newContext.currentElements.add(edge);
      } catch {
        // 忽略无效边
      }
    }

    return newContext;
  }

  // ============ 遍历步骤执行 ============

  /**
   * 执行 out() 步骤
   */
  private async executeOut(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);

        // 查询出边
        const criteria: any = { subjectId: nodeId };

        // 如果指定了边标签，添加谓词过滤
        if (step.edgeLabels && step.edgeLabels.length > 0) {
          for (const label of step.edgeLabels) {
            const predicateId = this.store.getNodeIdByValue(label);
            if (predicateId !== undefined) {
              criteria.predicateId = predicateId;
              break;
            }
          }
        }

        const records = this.store.resolveRecords(this.store.query(criteria), {
          includeProperties: false,
        });

        for (const record of records) {
          try {
            const targetVertex = this.adapter.nodeToVertex(record.objectId);
            newContext.currentElements.add(targetVertex);
          } catch {
            // 忽略无效目标顶点
          }
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 in() 步骤
   */
  private async executeIn(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);

        // 查询入边
        const criteria: any = { objectId: nodeId };

        if (step.edgeLabels && step.edgeLabels.length > 0) {
          for (const label of step.edgeLabels) {
            const predicateId = this.store.getNodeIdByValue(label);
            if (predicateId !== undefined) {
              criteria.predicateId = predicateId;
              break;
            }
          }
        }

        const records = this.store.resolveRecords(this.store.query(criteria), {
          includeProperties: false,
        });

        for (const record of records) {
          try {
            const sourceVertex = this.adapter.nodeToVertex(record.subjectId);
            newContext.currentElements.add(sourceVertex);
          } catch {
            // 忽略无效源顶点
          }
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 both() 步骤
   */
  private async executeBoth(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    // 合并 out 和 in 的结果
    const outContext = await this.executeOut(step, context);
    const inContext = await this.executeIn(step, context);

    const newContext = { ...context };
    newContext.currentElements = new Set([
      ...outContext.currentElements,
      ...inContext.currentElements,
    ]);

    return newContext;
  }

  /**
   * 执行 outE() 步骤
   */
  private async executeOutE(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);

        const criteria: any = { subjectId: nodeId };

        if (step.edgeLabels && step.edgeLabels.length > 0) {
          for (const label of step.edgeLabels) {
            const predicateId = this.store.getNodeIdByValue(label);
            if (predicateId !== undefined) {
              criteria.predicateId = predicateId;
              break;
            }
          }
        }

        const records = this.store.resolveRecords(this.store.query(criteria), {
          includeProperties: false,
        });

        for (const record of records) {
          try {
            const edge = this.adapter.tripleToEdge(
              record.subjectId,
              record.predicateId,
              record.objectId,
            );
            newContext.currentElements.add(edge);
          } catch {
            // 忽略无效边
          }
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 inE() 步骤
   */
  private async executeInE(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);

        const criteria: any = { objectId: nodeId };

        if (step.edgeLabels && step.edgeLabels.length > 0) {
          for (const label of step.edgeLabels) {
            const predicateId = this.store.getNodeIdByValue(label);
            if (predicateId !== undefined) {
              criteria.predicateId = predicateId;
              break;
            }
          }
        }

        const records = this.store.resolveRecords(this.store.query(criteria), {
          includeProperties: false,
        });

        for (const record of records) {
          try {
            const edge = this.adapter.tripleToEdge(
              record.subjectId,
              record.predicateId,
              record.objectId,
            );
            newContext.currentElements.add(edge);
          } catch {
            // 忽略无效边
          }
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 bothE() 步骤
   */
  private async executeBothE(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const outEContext = await this.executeOutE(step, context);
    const inEContext = await this.executeInE(step, context);

    const newContext = { ...context };
    newContext.currentElements = new Set([
      ...outEContext.currentElements,
      ...inEContext.currentElements,
    ]);

    return newContext;
  }

  /**
   * 执行 inV() 步骤
   */
  private async executeInV(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'edge') {
        const edge = element as Edge;
        try {
          const inVertex = this.adapter.nodeToVertex(Number(edge.inVertex));
          newContext.currentElements.add(inVertex);
        } catch {
          // 忽略无效顶点
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 outV() 步骤
   */
  private async executeOutV(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'edge') {
        const edge = element as Edge;
        try {
          const outVertex = this.adapter.nodeToVertex(Number(edge.outVertex));
          newContext.currentElements.add(outVertex);
        } catch {
          // 忽略无效顶点
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 bothV() 步骤
   */
  private async executeBothV(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const inVContext = await this.executeInV(step, context);
    const outVContext = await this.executeOutV(step, context);

    const newContext = { ...context };
    newContext.currentElements = new Set([
      ...inVContext.currentElements,
      ...outVContext.currentElements,
    ]);

    return newContext;
  }

  // ============ 过滤步骤执行 ============

  /**
   * 执行 has() 步骤
   */
  private async executeHas(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);

        // 如果只检查属性是否存在
        if (step.key && step.value === undefined && !step.predicate) {
          const predicateId = this.store.getNodeIdByValue(step.key);
          if (predicateId !== undefined) {
            const records = this.store.resolveRecords(
              this.store.query({ subjectId: nodeId, predicateId }),
              { includeProperties: false },
            );
            if (records.length > 0) {
              newContext.currentElements.add(element);
            }
          }
        }
        // 如果检查属性值
        else if (step.key && (step.value !== undefined || step.predicate)) {
          const predicateId = this.store.getNodeIdByValue(step.key);
          if (predicateId !== undefined) {
            const records = this.store.resolveRecords(
              this.store.query({ subjectId: nodeId, predicateId }),
              { includeProperties: false },
            );

            for (const record of records) {
              const objectValue = this.store.getNodeValueById(record.objectId);

              if (step.value !== undefined) {
                if (objectValue === step.value) {
                  newContext.currentElements.add(element);
                  break;
                }
              } else if (step.predicate) {
                if (
                  this.evaluatePredicate(
                    { properties: { value: objectValue } } as any,
                    step.predicate,
                  )
                ) {
                  newContext.currentElements.add(element);
                  break;
                }
              }
            }
          }
        }
        // 回退到原有逻辑（用于其他情况）
        else if (this.matchesHasFilter(element, step)) {
          newContext.currentElements.add(element);
        }
      } else {
        // 对于边，使用原有过滤逻辑
        if (this.matchesHasFilter(element, step)) {
          newContext.currentElements.add(element);
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 hasLabel() 步骤
   */
  private async executeHasLabel(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (step.labels.includes(element.label)) {
        newContext.currentElements.add(element);
      }
    }

    return newContext;
  }

  /**
   * 执行 hasId() 步骤
   */
  private async executeHasId(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (step.ids.includes(element.id)) {
        newContext.currentElements.add(element);
      }
    }

    return newContext;
  }

  /**
   * 执行 is() 步骤
   */
  private async executeIs(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (this.evaluatePredicate(element, step.predicate)) {
        newContext.currentElements.add(element);
      }
    }

    return newContext;
  }

  // ============ 范围限制步骤执行 ============

  /**
   * 执行 limit() 步骤
   */
  private async executeLimit(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const elements = Array.from(context.currentElements);
    newContext.currentElements = new Set(elements.slice(0, step.limit));
    return newContext;
  }

  /**
   * 执行 range() 步骤
   */
  private async executeRange(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const elements = Array.from(context.currentElements);
    newContext.currentElements = new Set(elements.slice(step.low, step.high));
    return newContext;
  }

  /**
   * 执行 skip() 步骤
   */
  private async executeSkip(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const elements = Array.from(context.currentElements);
    newContext.currentElements = new Set(elements.slice(step.skip));
    return newContext;
  }

  /**
   * 执行 dedup() 步骤
   */
  private async executeDedup(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const seen = new Set<string>();
    const dedupElements = new Set<Vertex | Edge>();

    for (const element of context.currentElements) {
      const key = this.getElementKey(element);
      if (!seen.has(key)) {
        seen.add(key);
        dedupElements.add(element);
      }
    }

    newContext.currentElements = dedupElements;
    return newContext;
  }

  // ============ 标记和选择步骤执行 ============

  /**
   * 执行 as() 步骤
   */
  private async executeAs(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.labeledElements.set(step.stepLabel, new Set(context.currentElements));
    return newContext;
  }

  /**
   * 执行 select() 步骤
   */
  private async executeSelect(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const key of step.selectKeys) {
      const labeledElements = context.labeledElements.get(key);
      if (labeledElements) {
        for (const element of labeledElements) {
          newContext.currentElements.add(element);
        }
      }
    }

    return newContext;
  }

  // ============ 投影步骤执行 ============

  /**
   * 执行 values() 步骤
   */
  private async executeValues(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      if (element.type === 'vertex') {
        const vertex = element as Vertex;
        const nodeId = Number(vertex.id);
        const keys = step.propertyKeys || ['HAS_NAME']; // 默认查找名称属性

        for (const key of keys) {
          const predicateId = this.store.getNodeIdByValue(key);
          if (predicateId !== undefined) {
            const records = this.store.resolveRecords(
              this.store.query({ subjectId: nodeId, predicateId }),
              { includeProperties: false },
            );

            for (const record of records) {
              const objectValue = this.store.getNodeValueById(record.objectId);
              const valueElement = {
                type: 'value',
                id: `${element.id}_${key}_${record.objectId}`,
                label: 'value',
                properties: { value: objectValue },
              } as any;
              newContext.currentElements.add(valueElement);
            }
          }
        }
      } else {
        // 对于边，使用原有逻辑
        const keys = step.propertyKeys || Object.keys(element.properties);
        for (const key of keys) {
          if (element.properties[key] !== undefined) {
            const valueElement = {
              type: 'value',
              id: `${element.id}_${key}`,
              label: 'value',
              properties: { value: element.properties[key] },
            } as any;
            newContext.currentElements.add(valueElement);
          }
        }
      }
    }

    return newContext;
  }

  /**
   * 执行 valueMap() 步骤
   */
  private async executeValueMap(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      const keys = step.propertyKeys || Object.keys(element.properties);
      const valueMap: Record<string, PropertyValue> = {};

      for (const key of keys) {
        if (element.properties[key] !== undefined) {
          valueMap[key] = element.properties[key];
        }
      }

      const mapElement = {
        type: 'map',
        id: `${element.id}_map`,
        label: 'map',
        properties: valueMap,
      } as any;

      newContext.currentElements.add(mapElement);
    }

    return newContext;
  }

  /**
   * 执行 elementMap() 步骤
   */
  private async executeElementMap(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    newContext.currentElements = new Set();

    for (const element of context.currentElements) {
      const keys = step.propertyKeys || Object.keys(element.properties);
      const elementMap: Record<string, PropertyValue> = {
        id: element.id,
        label: element.label,
      };

      for (const key of keys) {
        if (element.properties[key] !== undefined) {
          elementMap[key] = element.properties[key];
        }
      }

      const mapElement = {
        type: 'elementMap',
        id: `${element.id}_elementMap`,
        label: 'elementMap',
        properties: elementMap,
      } as any;

      newContext.currentElements.add(mapElement);
    }

    return newContext;
  }

  // ============ 聚合步骤执行 ============

  /**
   * 执行 count() 步骤
   */
  private async executeCount(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const count = context.currentElements.size;

    const countElement = {
      type: 'count',
      id: 'count',
      label: 'count',
      properties: { value: count },
    } as any;

    newContext.currentElements = new Set([countElement]);
    return newContext;
  }

  /**
   * 执行 fold() 步骤
   */
  private async executeFold(step: any, context: ExecutionContext): Promise<ExecutionContext> {
    const newContext = { ...context };
    const elements = Array.from(context.currentElements);

    const foldElement = {
      type: 'list',
      id: 'fold',
      label: 'list',
      properties: { value: elements },
    } as any;

    newContext.currentElements = new Set([foldElement]);
    return newContext;
  }

  // ============ 工具方法 ============

  /**
   * 获取元素唯一键
   */
  private getElementKey(element: Vertex | Edge): string {
    return `${element.type}_${element.id}`;
  }

  /**
   * 检查元素是否匹配 has 过滤器
   */
  private matchesHasFilter(element: Vertex | Edge, step: any): boolean {
    if (step.key) {
      const value = element.properties[step.key];

      if (step.value !== undefined) {
        return value === step.value;
      }

      if (step.predicate) {
        return this.evaluatePredicate({ properties: { value } } as any, step.predicate);
      }

      return value !== undefined;
    }

    return true;
  }

  /**
   * 评估谓词
   */
  private evaluatePredicate(element: any, predicate: Predicate): boolean {
    const value = element.properties?.value ?? element;

    switch (predicate.operator) {
      case P.eq:
        return value === predicate.value;
      case P.neq:
        return value !== predicate.value;
      case P.lt:
        return value < (predicate.value as any);
      case P.lte:
        return value <= (predicate.value as any);
      case P.gt:
        return value > (predicate.value as any);
      case P.gte:
        return value >= (predicate.value as any);
      case P.within:
        return Array.isArray(predicate.value) && predicate.value.includes(value);
      case P.without:
        return Array.isArray(predicate.value) && !predicate.value.includes(value);
      case P.between:
        return value >= (predicate.value as any) && value < (predicate.other as any);
      case P.inside:
        return value > (predicate.value as any) && value < (predicate.other as any);
      case P.outside:
        return value < (predicate.value as any) || value > (predicate.other as any);
      default:
        return false;
    }
  }
}
