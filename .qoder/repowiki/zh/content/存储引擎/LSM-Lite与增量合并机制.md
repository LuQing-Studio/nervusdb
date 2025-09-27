
# LSM-Lite与增量合并机制

<cite>
**本文档引用的文件**
- [staging.ts](file://src/storage/staging.ts)
- [compaction.ts](file://src/maintenance/compaction.ts)
- [persistentStore.ts](file://src/storage/persistentStore.ts)
- [openOptions.ts](file://src/types/openOptions.ts)
- [lsm_lite_staging.test.ts](file://tests/integration/storage/lsm_lite_staging.test.ts)
- [lsm_compaction_merge.test.ts](file://tests/integration/storage/lsm_compaction_merge.test.ts)
- [auto_compact_lsm_merge.test.ts](file://tests/integration/maintenance/auto_compact_lsm_merge.test.ts)
</cite>

## 目录
1. [简介](#简介)
2. [LSM-Lite暂存机制](#lsm-lite暂存机制)
3. [段合并（Compaction）过程](#段合并compaction过程)
4. [增量合并策略](#增量合并策略)
5. [自动合并触发逻辑](#自动合并触发逻辑)
6. [读快照一致性与并发安全](#读快照一致性与并发安全)
7. [运维调优建议](#运维调优建议)
8. [结论](#结论)

## 简介
本项目实现了一种基于LSM-Lite风格的轻量级写优化存储架构，通过内存暂存（staging area）和异步持久化机制提升小批量写入性能。系统采用分层设计，将高频写操作暂存于内存段，随后通过异步flush操作将其持久化为只读段文件。为了管理数据碎片并回收被删除记录占用的空间，系统实现了段合并（compaction）机制，该机制不仅能减少存储碎片、提升查询效率，还能有效平滑I/O负载。

核心组件包括`LsmLiteStaging`类用于管理内存中的暂存段，以及`compactDatabase`函数负责执行段合并操作。系统支持两种合并模式：重写（rewrite）和增量（incremental），后者特别适用于高并发场景以降低对查询性能的影响。此外，通过集成测试验证了在合并过程中读取快照的一致性及多线程访问的安全性，确保了系统的可靠性和稳定性。

## LSM-Lite暂存机制

`LsmLiteStaging`类是LSM-Lite风格暂存机制的核心实现，它提供了一个简单的接口来管理内存中的暂存段。当启用`stagingMode: 'lsm-lite'`时，所有新增的事实（facts）不仅会被添加到默认的内存索引中，还会被复制到`LsmLiteStaging`实例的memtable中。这一设计允许系统在未来无缝切换至完全基于LSM的写路径，而当前阶段主要作为旁路收集器使用，不影响现有的查询可见性。

在每次调用`flush()`方法时，系统会检查是否存在未处理的暂存数据，并将其写入磁盘上的段文件。这些段文件随后可以被纳入后续的合并流程中，从而实现从内存到磁盘再到最终归档的完整生命周期管理。此机制显著提高了写入吞吐量，同时保持了良好的查询性能。

**Section sources**
- [staging.ts](file://src/storage/staging.ts#L0-L30)
- [persistentStore.ts](file://src/storage/persistentStore.ts#L1590-L1600)
- [openOptions.ts](file://src/types/openOptions.ts#L120-L130)

## 段合并（Compaction）过程

段合并是维护数据库健康状态的关键操作，旨在减少数据碎片、回收空间并提高查询效率。`compactDatabase`函数实现了这一过程，其工作原理如下：

首先，函数读取当前数据库的状态信息，包括各个索引顺序（如SPO, SOP等）的页面分布情况。接着，根据配置选项评估是否需要进行合并，判断依据包括每个主键对应的页数是否达到预设阈值或墓碑（tombstone）比例是否过高。一旦决定执行合并，系统将按照指定模式处理数据。

对于“重写”模式，整个索引文件将被重新构建；而对于“增量”模式，则仅针对满足条件的主键生成新的页面，并更新manifest映射关系。无论哪种模式，最终都会产生一个包含最新数据布局的新manifest文件，标志着合并完成。此外，若启用了`includeLsmSegments