/**
 * 双向 BFS 变长路径查询优化
 *
 * 对于点到点的最短路径查询，双向 BFS 可以显著减少搜索空间
 * 时间复杂度从 O(b^d) 减少到 O(b^(d/2))，其中 b 是分支因子，d 是深度
 */

import { PersistentStore, FactRecord } from '../../core/storage/persistentStore.js';
import type {
  Uniqueness,
  Direction,
  PathEdge,
  PathResult,
  VariablePathOptions,
} from './variable.js';

interface BidirectionalState {
  node: number;
  edges: PathEdge[];
  visitedNodes: Set<number>;
  visitedEdges: Set<string>;
  depth: number;
}

interface IntersectionPoint {
  forwardState: BidirectionalState;
  backwardState: BidirectionalState;
  meetingNode: number;
}

export class BidirectionalPathBuilder {
  constructor(
    private readonly store: PersistentStore,
    private readonly startNodes: Set<number>,
    private readonly targetNodes: Set<number>,
    private readonly predicateId: number,
    private readonly options: VariablePathOptions,
  ) {}

  private neighbors(nodeId: number, dir: Direction): FactRecord[] {
    const crit =
      dir === 'forward'
        ? { subjectId: nodeId, predicateId: this.predicateId }
        : { predicateId: this.predicateId, objectId: nodeId };
    const enc = this.store.query(crit);
    return this.store.resolveRecords(enc);
  }

  private nextNode(rec: FactRecord, from: number, dir: Direction): number {
    if (dir === 'forward') return rec.objectId;
    return rec.subjectId;
  }

  private reverseDirection(dir: Direction): Direction {
    return dir === 'forward' ? 'reverse' : 'forward';
  }

  /**
   * 双向 BFS 最短路径查询
   * 从起点和终点同时开始搜索，直到两个搜索前沿相遇
   */
  shortestPath(): PathResult | null {
    const min = Math.max(1, this.options.min ?? 1);
    const max = Math.max(min, this.options.max);
    const dir = this.options.direction ?? 'forward';
    const uniq = this.options.uniqueness ?? 'NODE';

    // 如果最小长度为 0 或起点和终点有交集，直接返回
    if (min === 0) {
      for (const start of this.startNodes) {
        if (this.targetNodes.has(start)) {
          return {
            edges: [],
            length: 0,
            startId: start,
            endId: start,
          };
        }
      }
    }

    // 初始化前向和后向搜索队列
    const forwardQueue: BidirectionalState[] = [];
    const backwardQueue: BidirectionalState[] = [];

    // 前向搜索已访问的节点映射 (nodeId -> state)
    const forwardVisited = new Map<number, BidirectionalState>();
    // 后向搜索已访问的节点映射 (nodeId -> state)
    const backwardVisited = new Map<number, BidirectionalState>();

    // 初始化起点
    for (const start of this.startNodes) {
      const state: BidirectionalState = {
        node: start,
        edges: [],
        visitedNodes: new Set([start]),
        visitedEdges: new Set(),
        depth: 0,
      };
      forwardQueue.push(state);
      forwardVisited.set(start, state);
    }

    // 初始化终点（后向搜索）
    for (const target of this.targetNodes) {
      const state: BidirectionalState = {
        node: target,
        edges: [],
        visitedNodes: new Set([target]),
        visitedEdges: new Set(),
        depth: 0,
      };
      backwardQueue.push(state);
      backwardVisited.set(target, state);
    }

    let currentDepth = 0;
    const maxDepth = Math.floor(max / 2) + 1; // 双向搜索的最大深度

    while (currentDepth <= maxDepth && (forwardQueue.length > 0 || backwardQueue.length > 0)) {
      // 检查是否找到交集
      const intersection = this.findIntersection(forwardVisited, backwardVisited, min);
      if (intersection) {
        return this.buildPath(intersection, dir);
      }

      // 扩展两个方向的队列
      if (forwardQueue.length > 0) {
        this.expandQueue(forwardQueue, forwardVisited, dir, uniq, currentDepth, maxDepth);
      }

      if (backwardQueue.length > 0) {
        this.expandQueue(
          backwardQueue,
          backwardVisited,
          this.reverseDirection(dir),
          uniq,
          currentDepth,
          maxDepth,
        );
      }

      currentDepth++;
    }

    return null; // 未找到路径
  }

  private expandQueue(
    queue: BidirectionalState[],
    visited: Map<number, BidirectionalState>,
    searchDir: Direction,
    uniq: Uniqueness,
    targetDepth: number,
    maxDepth: number,
  ): void {
    const newStates: BidirectionalState[] = [];

    // 只处理当前深度的状态
    while (queue.length > 0) {
      const current = queue.shift()!;

      if (current.depth !== targetDepth) {
        // 非当前深度的状态重新加入队列
        newStates.push(current);
        continue;
      }

      if (current.depth >= maxDepth) continue;

      for (const rec of this.neighbors(current.node, searchDir)) {
        const next = this.nextNode(rec, current.node, searchDir);
        const edgeKey = `${rec.subjectId}:${rec.predicateId}:${rec.objectId}`;

        // 唯一性检查
        if (uniq === 'NODE' && current.visitedNodes.has(next)) continue;
        if (uniq === 'EDGE' && current.visitedEdges.has(edgeKey)) continue;

        const nextEdges = [...current.edges, { record: rec, direction: searchDir }];
        const nextVisitedNodes = new Set(current.visitedNodes);
        nextVisitedNodes.add(next);
        const nextVisitedEdges = new Set(current.visitedEdges);
        nextVisitedEdges.add(edgeKey);

        const nextState: BidirectionalState = {
          node: next,
          edges: nextEdges,
          visitedNodes: nextVisitedNodes,
          visitedEdges: nextVisitedEdges,
          depth: current.depth + 1,
        };

        newStates.push(nextState);

        // 更新访问记录（允许更短路径覆盖）
        const existingState = visited.get(next);
        if (!existingState || nextState.depth < existingState.depth) {
          visited.set(next, nextState);
        }
      }
    }

    // 将新状态加回队列
    queue.push(...newStates);
  }

  private findIntersection(
    forwardVisited: Map<number, BidirectionalState>,
    backwardVisited: Map<number, BidirectionalState>,
    minLength: number,
  ): IntersectionPoint | null {
    // 查找两个搜索前沿的交集
    for (const [nodeId, forwardState] of forwardVisited.entries()) {
      const backwardState = backwardVisited.get(nodeId);
      if (backwardState) {
        const totalLength = forwardState.depth + backwardState.depth;
        if (totalLength >= minLength) {
          return {
            forwardState,
            backwardState,
            meetingNode: nodeId,
          };
        }
      }
    }
    return null;
  }

  private buildPath(intersection: IntersectionPoint, direction: Direction): PathResult {
    const { forwardState, backwardState, meetingNode } = intersection;

    // 构建完整路径
    let fullEdges: PathEdge[] = [];
    let startId = meetingNode;
    let endId = meetingNode;

    // 添加前向路径
    if (forwardState.edges.length > 0) {
      fullEdges = [...forwardState.edges];
      startId = forwardState.edges[0].record.subjectId;
    }

    // 添加后向路径（需要反转）
    if (backwardState.edges.length > 0) {
      const reversedEdges = [...backwardState.edges].reverse().map((edge) => ({
        record: edge.record,
        direction: this.reverseDirection(edge.direction),
      }));
      fullEdges = [...fullEdges, ...reversedEdges];
      endId = backwardState.edges[0].record.subjectId;
    }

    return {
      edges: fullEdges,
      length: fullEdges.length,
      startId,
      endId,
    };
  }

  /**
   * 双向 BFS 所有路径查询（限制在合理的搜索范围内）
   */
  allPaths(): PathResult[] {
    // 对于所有路径查询，双向 BFS 的优势不如单向 BFS 明显
    // 这里提供基本实现，但建议对于复杂查询仍使用单向 BFS
    const shortestPath = this.shortestPath();
    return shortestPath ? [shortestPath] : [];
  }
}

/**
 * 便利函数：为现有 VariablePathBuilder 添加双向 BFS 优化
 */
export function createOptimizedPathBuilder(
  store: PersistentStore,
  startNodes: Set<number>,
  predicateId: number,
  options: VariablePathOptions & { target?: number },
): { shortest(): PathResult | null } {
  if (options.target !== undefined) {
    // 点到点查询，使用双向 BFS
    const bidirectional = new BidirectionalPathBuilder(
      store,
      startNodes,
      new Set([options.target]),
      predicateId,
      options,
    );
    return {
      shortest: () => bidirectional.shortestPath(),
    };
  } else {
    // 点到多点查询，回退到原始实现
    const { VariablePathBuilder } = require('./variable.js');
    const original = new VariablePathBuilder(store, startNodes, predicateId, options);
    return {
      shortest: () => original.shortest(options.target!),
    };
  }
}
