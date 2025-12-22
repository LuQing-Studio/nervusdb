# T4: Node 吞吐修复（批量返回字符串 triples）

## 1. Context

- 现状：Node 侧拿到 `query()` 返回的 ID 三元组后，会对每条 triple 触发 3 次跨 FFI 的 `resolveStr()`。
- 这会把性能拖死在 JS↔Native 调用开销上，属于“用脚写数据库”。

## 2. Goals

- Rust Native 一次性返回“已解析字符串”的 facts，避免 per-triple 的 3 次 resolve 往返。
- 兼容旧 addon：TypeScript 外壳根据“方法是否存在”做降级（不强行要求一次性升级所有用户）。
- 覆盖两种路径：
  - 普通查询（一次性返回）
  - 游标流式读取（避免一次性分配巨大数组）

## 3. Non-Goals

- 不做复杂的零拷贝二进制协议（那是过度工程化）。
- 不在 JS 侧做“大缓存字典镜像”（缓存一致性会把你拖进泥潭）。

## 4. Solution

### 4.1 Native (N-API) 新增接口

- `queryFacts(criteria?) -> Vec<FactOutput>`：
  - `FactOutput` 包含 `subject/predicate/object` 字符串，以及对应 ID（便于上层索引/属性操作）。
- `readCursorFacts(cursorId, batchSize) -> { facts: FactOutput[], done: bool }`：
  - 与现有 `readCursor` 并存，供 TS 外壳优先使用。

### 4.2 TypeScript 外壳改造

- `PersistentStore.query()` / `streamQuery()`：
  - 若 native handle 提供 `queryFacts/readCursorFacts`，直接使用；否则回退到旧逻辑（`query` + `resolveStr`）。

## 5. Testing Strategy

- Node 单测（mock native handle）：
  - 断言当 `queryFacts` 存在时，`resolveStr()` 不被调用。
  - 游标版本同理。

## 6. Risks

- 返回字符串会增加数据量：必须提供游标版接口，避免一次性爆内存。
- TS/NAPI 类型字段命名要一致（避免 `subject_id` vs `subjectId` 这种无聊坑）。

