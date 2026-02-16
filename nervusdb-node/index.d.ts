export type ScalarValue = null | boolean | number | string

export type ErrorCategory = 'syntax' | 'execution' | 'storage' | 'compatibility'

export interface NervusErrorPayload {
  code: string
  category: ErrorCategory
  message: string
}

export interface NodeValue {
  type: 'node'
  id: number
  labels: string[]
  properties: Record<string, unknown>
}

export interface RelationshipValue {
  type: 'relationship'
  src: number
  dst: number
  rel_type: string
  properties: Record<string, unknown>
}

export interface PathValue {
  type: 'path' | 'path_legacy'
  nodes: unknown[]
  relationships?: unknown[]
  edges?: unknown[]
}

export type QueryValue =
  | ScalarValue
  | NodeValue
  | RelationshipValue
  | PathValue
  | Record<string, unknown>
  | QueryValue[]

export type QueryRow = Record<string, QueryValue>
export type QueryParams = Record<string, QueryValue>

export interface VectorHit {
  nodeId: number
  distance: number
}

export interface VacuumReport {
  ndbPath: string
  backupPath: string
  oldNextPageId: number
  newNextPageId: number
  copiedDataPages: number
  oldFilePages: number
  newFilePages: number
}

export interface BackupInfo {
  id: string
  createdAt: string
  sizeBytes: number
  fileCount: number
  nervusdbVersion: string
  checkpointTxid: number
  checkpointEpoch: number
}

export interface BulkNodeInput {
  externalId: number
  label: string
  properties?: Record<string, QueryValue>
}

export interface BulkEdgeInput {
  srcExternalId: number
  relType: string
  dstExternalId: number
  properties?: Record<string, QueryValue>
}

export class Db {
  static open(path: string): Db
  static openPaths(ndbPath: string, walPath: string): Db

  readonly path: string
  readonly ndbPath: string
  readonly walPath: string

  query(cypher: string, params?: QueryParams): QueryRow[]
  executeWrite(cypher: string, params?: QueryParams): number

  beginWrite(): WriteTxn

  compact(): void
  checkpoint(): void
  createIndex(label: string, property: string): void
  searchVector(query: number[], k: number): VectorHit[]

  close(): void
}

export class WriteTxn {
  query(cypher: string, params?: QueryParams): void

  createNode(external_id: number, label_id: number): number
  getOrCreateLabel(name: string): number
  getOrCreateRelType(name: string): number
  createEdge(src: number, rel: number, dst: number): void
  tombstoneNode(node: number): void
  tombstoneEdge(src: number, rel: number, dst: number): void
  setNodeProperty(node: number, key: string, value: QueryValue): void
  setEdgeProperty(src: number, rel: number, dst: number, key: string, value: QueryValue): void
  removeNodeProperty(node: number, key: string): void
  removeEdgeProperty(src: number, rel: number, dst: number, key: string): void
  setVector(node: number, vector: number[]): void

  commit(): number
  rollback(): void
}

export function vacuum(path: string): VacuumReport
export function backup(path: string, backupDir: string): BackupInfo
export function bulkload(path: string, nodes: BulkNodeInput[], edges: BulkEdgeInput[]): void
