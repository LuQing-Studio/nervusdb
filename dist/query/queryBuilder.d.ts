import { FactInput, FactRecord } from '../storage/persistentStore.js';
import { PersistentStore } from '../storage/persistentStore.js';
export type FactCriteria = Partial<FactInput>;
export type FrontierOrientation = 'subject' | 'object' | 'both';
export interface PropertyFilter {
    propertyName: string;
    value?: unknown;
    range?: {
        min?: unknown;
        max?: unknown;
        includeMin?: boolean;
        includeMax?: boolean;
    };
}
interface QueryContext {
    facts: FactRecord[];
    frontier: Set<number>;
    orientation: FrontierOrientation;
}
export declare class QueryBuilder {
    private readonly store;
    private readonly facts;
    private readonly frontier;
    private readonly orientation;
    private readonly pinnedEpoch?;
    constructor(store: PersistentStore, context: QueryContext, pinnedEpoch?: number);
    get length(): number;
    slice(start?: number, end?: number): FactRecord[];
    [Symbol.iterator](): IterableIterator<FactRecord>;
    toArray(): FactRecord[];
    all(): FactRecord[];
    where(predicate: (record: FactRecord) => boolean): QueryBuilder;
    limit(n: number): QueryBuilder;
    /**
     * 根据节点属性过滤当前前沿
     * @param filter 属性过滤条件
     */
    whereNodeProperty(filter: PropertyFilter): QueryBuilder;
    /**
     * 根据边属性过滤当前事实
     * @param filter 属性过滤条件
     */
    whereEdgeProperty(filter: PropertyFilter): QueryBuilder;
    /**
     * 基于属性条件进行联想查询
     * @param predicate 关系谓词
     * @param nodePropertyFilter 可选的目标节点属性过滤条件
     */
    followWithNodeProperty(predicate: string, nodePropertyFilter?: PropertyFilter): QueryBuilder;
    /**
     * 基于属性条件进行反向联想查询
     * @param predicate 关系谓词
     * @param nodePropertyFilter 可选的目标节点属性过滤条件
     */
    followReverseWithNodeProperty(predicate: string, nodePropertyFilter?: PropertyFilter): QueryBuilder;
    /**
     * 带属性过滤的联想查询实现
     */
    private traverseWithProperty;
    anchor(orientation: FrontierOrientation): QueryBuilder;
    follow(predicate: string): QueryBuilder;
    followReverse(predicate: string): QueryBuilder;
    private traverse;
    static fromFindResult(store: PersistentStore, context: QueryContext, pinnedEpoch?: number): QueryBuilder;
    static empty(store: PersistentStore): QueryBuilder;
    private pin;
    private unpin;
}
export declare function buildFindContext(store: PersistentStore, criteria: FactCriteria, anchor: FrontierOrientation): QueryContext;
/**
 * 基于属性条件构建查询上下文
 * @param store 数据存储实例
 * @param propertyFilter 属性过滤条件
 * @param anchor 前沿方向
 * @param target 查询目标（节点或边）
 */
export declare function buildFindContextFromProperty(store: PersistentStore, propertyFilter: PropertyFilter, anchor: FrontierOrientation, target?: 'node' | 'edge'): QueryContext;
export {};
//# sourceMappingURL=queryBuilder.d.ts.map