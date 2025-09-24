/**
 * 全文搜索引擎入口
 *
 * 导出全文搜索功能的所有公共API和类型
 */

// 导出类型定义
export * from './types';

// 导出文本分析器
export {
  StandardAnalyzer,
  KeywordAnalyzer,
  NGramAnalyzer,
  AnalyzerFactory
} from './analyzer';

// 导出倒排索引和文档语料库
export {
  MemoryInvertedIndex,
  MemoryDocumentCorpus,
  BooleanQueryProcessor,
  PhraseQueryProcessor
} from './index';

// 导出相关性评分器
export {
  TFIDFScorer,
  BM25Scorer,
  FieldWeightedScorer,
  TimeDecayScorer,
  CompositeScorer,
  VectorSpaceScorer,
  ScorerFactory
} from './scorer';

// 导出查询引擎
export {
  EditDistanceCalculator,
  FuzzySearchProcessor,
  QueryParser,
  SearchHighlighter,
  FullTextQueryEngine
} from './query';

// 导出主搜索引擎
export {
  FullTextIndex,
  FullTextSearchEngine,
  FullTextSearchFactory,
  FullTextBatchProcessor,
  SearchPerformanceMonitor
} from './engine';