/**
 * 图算法库主入口
 *
 * 导出所有图算法相关的类型定义、实现和工具函数
 */

// 导出类型定义
export * from './types';

// 导出图数据结构
export {
  MemoryGraph,
  GraphBuilder
} from './graph';

// 导出中心性算法
export {
  PageRankCentrality,
  BetweennessCentrality,
  ClosenessCentrality,
  DegreeCentrality,
  EigenvectorCentrality,
  CentralityAlgorithmFactory
} from './centrality';

// 导出社区发现算法
export {
  LouvainCommunityDetection,
  LabelPropagationCommunityDetection,
  ConnectedComponentsDetection,
  StronglyConnectedComponentsDetection,
  CommunityDetectionAlgorithmFactory
} from './community';

// 导出路径算法
export {
  DijkstraPathAlgorithm,
  AStarPathAlgorithm,
  FloydWarshallPathAlgorithm,
  BellmanFordPathAlgorithm,
  PathAlgorithmFactory
} from './pathfinding';

// 导出相似度算法
export {
  JaccardSimilarity,
  CosineSimilarity,
  AdamicAdarSimilarity,
  PreferentialAttachmentSimilarity,
  SimRankSimilarity,
  NodeAttributeSimilarity,
  SimilarityAlgorithmFactory
} from './similarity';

// 导出统一算法套件
export {
  GraphAlgorithmSuiteImpl,
  GraphAlgorithmFactoryImpl,
  GraphAlgorithmUtils,
  GraphAlgorithms
} from './suite';

// 便捷API
export const createGraph = () => new (require('./graph').MemoryGraph)();
export const createGraphBuilder = () => new (require('./graph').GraphBuilder)();
export const createAlgorithmSuite = (graph: any) => new (require('./suite').GraphAlgorithmSuiteImpl)(graph);