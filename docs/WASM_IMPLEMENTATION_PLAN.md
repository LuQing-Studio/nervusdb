# NervusDB WebAssembly 实施计划

## 📋 项目概述

**目标**：将 NervusDB 核心存储引擎用 Rust 重写并编译成 WebAssembly，实现：

1. 🔒 **代码保护**：真正的二进制保护，极难反编译
2. ⚡ **性能提升**：20-50% 性能提升
3. 🐛 **内存泄漏修复**：Rust 的内存安全机制彻底解决 JavaScript 内存泄漏问题

**版本**：v1.2.0

**预计时间**：2-3 周

---

## 🎯 Phase 1: 内存泄漏分析与定位 (Day 1-2)

### 1.1 问题症状

**用户报告**：

- 长时间运行后电脑变慢
- 内存占用持续增长
- 可能的 GC 压力

### 1.2 诊断工具

```bash
# 1. Node.js 内存分析
node --expose-gc --inspect your-app.js

# 2. 使用 heapdump
npm install --save-dev heapdump
```

**测试脚本** (`scripts/memory-leak-analysis.mjs`):

```javascript
import { NervusDB } from '../src/index.js';
import v8 from 'v8';
import { writeHeapSnapshot } from 'v8';

async function analyzeMemoryLeak() {
  console.log('🔍 Starting memory leak analysis...');
  const snapshots = [];

  for (let i = 0; i < 10; i++) {
    // 模拟长时间运行
    const db = new NervusDB('/tmp/test.db');
    await db.open();

    // 插入数据
    for (let j = 0; j < 1000; j++) {
      await db.add({
        subject: `s${j}`,
        predicate: 'p',
        object: `o${j}`,
      });
    }

    // 查询
    const results = await db.query().whereSubject('s500').execute();

    await db.close();

    // 强制 GC
    if (global.gc) {
      global.gc();
    }

    // 记录内存
    const mem = process.memoryUsage();
    snapshots.push({
      iteration: i,
      heapUsed: Math.round(mem.heapUsed / 1024 / 1024),
      external: Math.round(mem.external / 1024 / 1024),
    });

    console.log(`Iteration ${i}: Heap=${mem.heapUsed / 1024 / 1024}MB`);

    // 第5次和第10次生成heap snapshot
    if (i === 4 || i === 9) {
      writeHeapSnapshot(`heap-snapshot-${i}.heapsnapshot`);
    }
  }

  // 分析结果
  console.log('\n📊 Memory Growth Analysis:');
  console.table(snapshots);

  const firstHeap = snapshots[0].heapUsed;
  const lastHeap = snapshots[snapshots.length - 1].heapUsed;
  const growth = lastHeap - firstHeap;

  if (growth > 50) {
    console.log(`⚠️  Potential memory leak detected! Growth: ${growth}MB`);
  } else {
    console.log(`✅ Memory usage stable. Growth: ${growth}MB`);
  }
}

analyzeMemoryLeak().catch(console.error);
```

### 1.3 已知可疑点

基于代码审查，可能的内存泄漏源：

#### 1.3.1 文件句柄未关闭

**位置**：`src/storage/persistentStore.ts`

```typescript
// 🐛 问题：FileHandle 可能未正确关闭
async open(path: string) {
  this.dbFile = await fs.open(path, 'r+');
  // 如果后续操作失败，文件句柄可能泄漏
}
```

**修复方案**：确保所有 FileHandle 在 finally 块中关闭

#### 1.3.2 事件监听器未移除

**位置**：`src/synapseDb.ts`

```typescript
// 🐛 问题：process.on 监听器可能未清理
private setupCleanup() {
  process.on('exit', () => this.close());
  process.on('SIGINT', () => this.close());
}
```

**修复方案**：使用 `process.once` 或在 close 时 `removeListener`

#### 1.3.3 循环引用

**位置**：`src/query/queryBuilder.ts`

```typescript
// 🐛 问题：可能存在循环引用
class QueryBuilder {
  constructor(private store: PersistentStore) {
    this.store.activeQueries.add(this); // 循环引用？
  }
}
```

**修复方案**：使用 WeakMap 或及时清理引用

#### 1.3.4 大对象缓存未限制

**位置**：`src/storage/index.ts`

```typescript
// 🐛 问题：索引缓存可能无限增长
private cache: Map<string, IndexEntry> = new Map();
```

**修复方案**：使用 LRU 缓存（lru-cache）

### 1.4 诊断步骤

```bash
# Day 1
1. 运行内存分析脚本
   node --expose-gc scripts/memory-leak-analysis.mjs

2. 对比 heap snapshots
   - 使用 Chrome DevTools Memory Profiler
   - 找到未被回收的对象

3. 定位泄漏源
   - 检查文件句柄
   - 检查事件监听器
   - 检查循环引用

# Day 2
4. 创建修复PR
5. 验证修复效果
```

---

## 🦀 Phase 2: Rust 项目搭建 (Day 3-4)

### 2.1 项目结构

```
nervusdb-wasm/
├── Cargo.toml
├── src/
│   ├── lib.rs              # WASM 入口
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── btree.rs        # B-Tree 实现
│   │   ├── lsm.rs          # LSM Tree
│   │   └── wal.rs          # Write-Ahead Log
│   ├── index/
│   │   ├── mod.rs
│   │   └── spo_index.rs    # SPO 索引
│   └── utils/
│       ├── mod.rs
│       └── memory.rs       # 内存管理
├── tests/
│   └── integration_test.rs
└── benches/
    └── storage_bench.rs
```

### 2.2 Cargo.toml 配置

```toml
[package]
name = "nervusdb-wasm"
version = "1.2.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
getrandom = { version = "0.2", features = ["js"] }

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = "z"  # 优化文件大小
lto = true       # Link Time Optimization
codegen-units = 1

[profile.dev]
opt-level = 0
```

### 2.3 核心模块识别

#### 优先级1：存储引擎核心 ⭐⭐⭐⭐⭐

**TypeScript** → **Rust**

| TS 文件                          | Rust 模块         | 优先级 | 估计时间 |
| -------------------------------- | ----------------- | ------ | -------- |
| `src/storage/persistentStore.ts` | `storage/mod.rs`  | P0     | 3 天     |
| `src/storage/sstable.ts`         | `storage/lsm.rs`  | P0     | 2 天     |
| `src/storage/index.ts`           | `index/mod.rs`    | P0     | 2 天     |
| `src/storage/wal.ts`             | `storage/wal.rs`  | P1     | 2 天     |
| `src/utils/memoryManager.ts`     | `utils/memory.rs` | P2     | 1 天     |

**总计**：约 10 天纯开发时间

#### 不移植的模块

- `src/query/queryBuilder.ts` - 保留 JavaScript（API 层）
- `src/cli/` - 保留 JavaScript
- `src/plugins/` - 保留 JavaScript

**原因**：WASM 只负责核心存储，保持 API 在 JavaScript 更灵活

---

## 💻 Phase 3: Rust 核心实现 (Day 5-12)

### 3.1 存储引擎核心 (Day 5-7)

**目标**：实现 `PersistentStore` 的 Rust 版本

**`src/storage/mod.rs`**:

```rust
use wasm_bindgen::prelude::*;
use std::collections::HashMap;
use std::path::Path;

#[wasm_bindgen]
pub struct StorageEngine {
    path: String,
    data: HashMap<String, Vec<u8>>,
}

#[wasm_bindgen]
impl StorageEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(path: String) -> Result<StorageEngine, JsValue> {
        Ok(StorageEngine {
            path,
            data: HashMap::new(),
        })
    }

    #[wasm_bindgen]
    pub fn insert(&mut self, key: String, value: Vec<u8>) -> Result<(), JsValue> {
        self.data.insert(key, value);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get(&self, key: String) -> Option<Vec<u8>> {
        self.data.get(&key).cloned()
    }

    #[wasm_bindgen]
    pub fn delete(&mut self, key: String) -> Result<(), JsValue> {
        self.data.remove(&key);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn flush(&mut self) -> Result<(), JsValue> {
        // TODO: 写入磁盘
        Ok(())
    }
}
```

### 3.2 B-Tree 索引 (Day 8-9)

**`src/storage/btree.rs`**:

```rust
use std::cmp::Ordering;

pub struct BTree<K: Ord, V> {
    root: Option<Box<Node<K, V>>>,
    order: usize,
}

struct Node<K: Ord, V> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Node<K, V>>>,
    is_leaf: bool,
}

impl<K: Ord, V> BTree<K, V> {
    pub fn new(order: usize) -> Self {
        BTree { root: None, order }
    }

    pub fn insert(&mut self, key: K, value: V) {
        // B-Tree 插入逻辑
    }

    pub fn search(&self, key: &K) -> Option<&V> {
        // B-Tree 查找逻辑
    }
}
```

### 3.3 LSM Tree (Day 10-11)

**`src/storage/lsm.rs`**:

```rust
pub struct LSMTree {
    memtable: MemTable,
    sstables: Vec<SSTable>,
}

impl LSMTree {
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.memtable.put(key, value);
        if self.memtable.size() > THRESHOLD {
            self.flush();
        }
    }

    fn flush(&mut self) {
        let sstable = SSTable::from_memtable(&self.memtable);
        self.sstables.push(sstable);
        self.memtable.clear();
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        // 先查 memtable
        if let Some(value) = self.memtable.get(key) {
            return Some(value);
        }

        // 再查 sstables（从新到旧）
        for sstable in self.sstables.iter().rev() {
            if let Some(value) = sstable.get(key) {
                return Some(value);
            }
        }

        None
    }
}
```

### 3.4 WAL (Day 12)

**`src/storage/wal.rs`**:

```rust
pub struct WriteAheadLog {
    file: std::fs::File,
}

impl WriteAheadLog {
    pub fn append(&mut self, operation: Operation) -> Result<(), Error> {
        let serialized = bincode::serialize(&operation)?;
        self.file.write_all(&serialized)?;
        self.file.sync_all()?;
        Ok(())
    }

    pub fn replay(&self) -> Result<Vec<Operation>, Error> {
        // 读取并重放所有操作
        todo!()
    }
}
```

---

## 🔗 Phase 4: TypeScript 集成 (Day 13-15)

### 4.1 编译 WASM

```bash
cd nervusdb-wasm
wasm-pack build --target nodejs --out-dir ../src/wasm
```

**输出**：

```
src/wasm/
├── nervusdb_wasm.js      # JavaScript 绑定
├── nervusdb_wasm_bg.wasm # WASM 二进制
├── nervusdb_wasm.d.ts    # TypeScript 类型
└── package.json
```

### 4.2 TypeScript 适配层

**`src/storage/persistentStoreWasm.ts`** (新文件):

```typescript
import init, { StorageEngine } from '../wasm/nervusdb_wasm.js';

let wasmInitialized = false;

async function ensureWasmInit() {
  if (!wasmInitialized) {
    await init();
    wasmInitialized = true;
  }
}

export class PersistentStoreWasm {
  private engine: StorageEngine | null = null;

  async open(path: string): Promise<void> {
    await ensureWasmInit();
    this.engine = new StorageEngine(path);
  }

  async add(triple: Triple): Promise<void> {
    if (!this.engine) throw new Error('Not initialized');

    const key = this.encodeKey(triple);
    const value = this.encodeValue(triple);
    this.engine.insert(key, value);
  }

  async find(pattern: TriplePattern): Promise<Triple[]> {
    if (!this.engine) throw new Error('Not initialized');

    // 使用 WASM 引擎查询
    const results: Triple[] = [];
    // ...
    return results;
  }

  private encodeKey(triple: Triple): string {
    return `${triple.subject}:${triple.predicate}:${triple.object}`;
  }

  private encodeValue(triple: Triple): Uint8Array {
    return new TextEncoder().encode(JSON.stringify(triple));
  }

  async close(): Promise<void> {
    if (this.engine) {
      this.engine.free(); // 释放 WASM 内存
      this.engine = null;
    }
  }
}
```

### 4.3 渐进式迁移策略

**`src/storage/persistentStore.ts`** (修改):

```typescript
import { PersistentStoreWasm } from './persistentStoreWasm.js';
import { PersistentStoreJS } from './persistentStoreJS.js';

export class PersistentStore {
  private backend: PersistentStoreWasm | PersistentStoreJS;

  constructor(private useWasm: boolean = true) {
    this.backend = useWasm ? new PersistentStoreWasm() : new PersistentStoreJS();
  }

  async open(path: string): Promise<void> {
    return this.backend.open(path);
  }

  async add(triple: Triple): Promise<void> {
    return this.backend.add(triple);
  }

  async find(pattern: TriplePattern): Promise<Triple[]> {
    return this.backend.find(pattern);
  }

  async close(): Promise<void> {
    return this.backend.close();
  }
}
```

**环境变量控制**:

```typescript
const useWasm = process.env.NERVUSDB_USE_WASM !== 'false';
```

---

## 🧪 Phase 5: 测试与验证 (Day 16-18)

### 5.1 功能测试

```bash
# 所有现有测试应该通过
NERVUSDB_USE_WASM=true pnpm test

# 对比 JavaScript 和 WASM 实现
NERVUSDB_USE_WASM=false pnpm test
NERVUSDB_USE_WASM=true pnpm test
```

### 5.2 性能基准测试

**`benchmarks/wasm-vs-js.mjs`**:

```javascript
import Benchmark from 'benchmark';
import { NervusDB } from '../src/index.js';

const suite = new Benchmark.Suite();

suite
  .add('JavaScript Backend', {
    defer: true,
    fn: async (deferred) => {
      const db = new NervusDB('/tmp/bench-js.db', { useWasm: false });
      await db.open();
      for (let i = 0; i < 1000; i++) {
        await db.add({ subject: `s${i}`, predicate: 'p', object: `o${i}` });
      }
      await db.close();
      deferred.resolve();
    },
  })
  .add('WASM Backend', {
    defer: true,
    fn: async (deferred) => {
      const db = new NervusDB('/tmp/bench-wasm.db', { useWasm: true });
      await db.open();
      for (let i = 0; i < 1000; i++) {
        await db.add({ subject: `s${i}`, predicate: 'p', object: `o${i}` });
      }
      await db.close();
      deferred.resolve();
    },
  })
  .on('cycle', (event) => {
    console.log(String(event.target));
  })
  .on('complete', function () {
    console.log('Fastest is ' + this.filter('fastest').map('name'));
  })
  .run({ async: true });
```

### 5.3 内存泄漏验证

```bash
# 运行修复后的内存分析
node --expose-gc scripts/memory-leak-analysis.mjs

# 长时间运行测试（24小时）
NERVUSDB_USE_WASM=true node tests/long-running.mjs
```

**预期结果**：

- ✅ 内存增长 < 10MB（10 次迭代）
- ✅ 无 FileHandle 泄漏警告
- ✅ Heap snapshot 对比：无异常增长对象

---

## 📊 Phase 6: 性能优化 (Day 19-20)

### 6.1 WASM 优化技巧

#### 6.1.1 减小 WASM 文件大小

```bash
# 使用 wasm-opt (binaryen)
wasm-opt -Oz input.wasm -o output.wasm

# 预期：从 500KB 压缩到 200KB
```

#### 6.1.2 内存对齐优化

```rust
#[repr(C)]
pub struct Triple {
    subject: u64,
    predicate: u32,
    object: u64,
}
```

#### 6.1.3 批量操作

```typescript
// ❌ 慢：每次调用都跨 WASM 边界
for (const triple of triples) {
  await store.add(triple);
}

// ✅ 快：批量传输
await store.addBatch(triples);
```

### 6.2 预期性能提升

| 操作       | JavaScript | WASM  | 提升   |
| ---------- | ---------- | ----- | ------ |
| 插入 1K 条 | 100ms      | 50ms  | 2x     |
| 查询       | 10ms       | 5ms   | 2x     |
| 索引构建   | 500ms      | 250ms | 2x     |
| 压缩       | 1000ms     | 600ms | 1.67x  |
| **平均**   | -          | -     | **2x** |

---

## 🚀 Phase 7: 发布与文档 (Day 21)

### 7.1 更新文档

- `README.md` - 添加 WASM 说明
- `docs/WASM_IMPLEMENTATION_PLAN.md` - 实施总结
- `docs/PERFORMANCE.md` - 性能对比

### 7.2 发布 v1.2.0

```bash
# 更新版本
npm version 1.2.0

# 构建
pnpm build

# 发布
npm publish
```

### 7.3 changelog

```markdown
## v1.2.0 - WebAssembly Storage Engine (2025-01-XX)

### ⚡ Performance

- **2x faster** storage operations with Rust+WASM backend
- Reduced memory footprint by 30%

### 🐛 Bug Fixes

- **CRITICAL**: Fixed memory leak in file handle management
- Fixed event listener cleanup issues
- Fixed circular reference in query builder

### 🔒 Security

- Core storage engine compiled to WebAssembly (binary protection)
- Harder to reverse engineer than JavaScript

### ✨ Features

- New `useWasm` option (default: true)
- Backward compatible with JavaScript backend
- Automatic fallback if WASM not available

### 📦 Package Changes

- Added `nervusdb-wasm.wasm` (200KB)
- Total package size: ~400KB (was 150KB)
```

---

## ✅ 成功标准

### 必须达成 (P0)

- [ ] 所有 548 个测试通过（WASM 模式）
- [ ] 性能提升 >= 20%
- [ ] 内存泄漏修复：长时间运行内存增长 < 10MB
- [ ] WASM 文件大小 < 300KB
- [ ] 向后兼容：支持禁用 WASM

### 应该达成 (P1)

- [ ] 性能提升 >= 50%
- [ ] WASM 文件大小 < 200KB
- [ ] 文档完整（README + API docs）
- [ ] 基准测试报告

### 可以达成 (P2)

- [ ] 支持浏览器环境
- [ ] 提供 WASM 调试模式
- [ ] 性能监控仪表板

---

## 🚨 风险与缓解

### 风险 1：Rust 学习曲线

**影响**：中等  
**概率**：高

**缓解措施**：

- 先从简单模块开始（BTree → LSM → WAL）
- 参考优秀的 Rust 数据库项目（sled, rocksdb-rust）
- 使用 ChatGPT/Gemini 辅助编写 Rust 代码

### 风险 2：WASM 性能不如预期

**影响**：高  
**概率**：低

**缓解措施**：

- 保留 JavaScript 后端作为 fallback
- 只移植性能关键模块
- 使用 `wasm-pack` 最佳实践

### 风险 3：WASM 文件过大

**影响**：中等  
**概率**：中等

**缓解措施**：

- 使用 `wasm-opt -Oz` 压缩
- 只编译必要功能
- 考虑按需加载（懒加载）

### 风险 4：跨平台兼容性问题

**影响**：高  
**概率**：低

**缓解措施**：

- 在多个平台测试（macOS/Linux/Windows）
- 使用 CI/CD 自动化测试
- 提供 JavaScript fallback

---

## 📚 参考资源

### Rust + WebAssembly

- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [wasm-pack Documentation](https://rustwasm.github.io/docs/wasm-pack/)

### 数据库实现

- [sled](https://github.com/spacejam/sled) - Pure Rust 嵌入式数据库
- [RocksDB](https://github.com/facebook/rocksdb) - LSM Tree 实现
- [SQLite WASM](https://github.com/sql-js/sql.js) - WASM 案例

### 性能优化

- [WASM Performance Guide](https://rustwasm.github.io/book/reference/code-size.html)
- [binaryen wasm-opt](https://github.com/WebAssembly/binaryen)

---

## 📅 时间表

| 阶段               | 天数   | 日期范围  | 负责人 |
| ------------------ | ------ | --------- | ------ |
| Phase 1: 内存分析  | 2 天   | Day 1-2   | Droid  |
| Phase 2: 项目搭建  | 2 天   | Day 3-4   | Droid  |
| Phase 3: Rust 实现 | 8 天   | Day 5-12  | Droid  |
| Phase 4: TS 集成   | 3 天   | Day 13-15 | Droid  |
| Phase 5: 测试验证  | 3 天   | Day 16-18 | Droid  |
| Phase 6: 性能优化  | 2 天   | Day 19-20 | Droid  |
| Phase 7: 发布文档  | 1 天   | Day 21    | Droid  |
| **总计**           | **21** | **3 周**  | -      |

---

## 🎬 下一步行动

### 立即开始（今天）

```bash
# 1. 运行内存分析
node --expose-gc scripts/memory-leak-analysis.mjs

# 2. 创建 Rust 项目
mkdir nervusdb-wasm
cd nervusdb-wasm
cargo init --lib
```

### 明天

- 完成内存泄漏定位
- 修复 JavaScript 版本的内存泄漏
- 验证修复效果

### 本周内

- Rust 项目搭建完成
- 存储引擎核心 Rust 实现完成
- 编译出第一个 WASM 模块

---

**准备好了吗？让我们开始！🚀**
