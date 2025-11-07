# Architecture Decision Records (ADR) 索引

本目录包含项目的所有架构决策记录。

## ADR 清单

| 编号                                          | 主题       | 状态   | 最后更新   | 关联Issue |
| --------------------------------------------- | ---------- | ------ | ---------- | --------- |
| [ADR-001](./ADR-001-Tech-Stack.md)            | 技术栈选型 | active | 2024-10-17 | -         |
| [ADR-002](./ADR-002-Architecture-Design.md)   | 架构设计   | active | 2024-10-17 | -         |
| [ADR-003](./ADR-003-Quality-Assurance.md)     | 质量保障   | active | 2024-10-17 | -         |
| [ADR-004](./ADR-004-Architecture-Upgrade.md)  | 架构升级   | active | 2024-10-17 | -         |
| [ADR-005](./ADR-005-Code-Organization.md)     | 代码组织   | active | 2024-11-06 | -         |
| [ADR-006](./ADR-006-Temporal-Memory-Graph.md) | 时序记忆图 | active | 2024-11-06 | -         |

## 状态说明

- **draft**: 草稿，尚未验证
- **active**: 已生效，正在使用
- **deprecated**: 已废弃，被新决策替代
- **superseded**: 被其他 ADR 替代

## 如何添加新的 ADR

1. 创建新文件：`ADR-XXX-<主题>.md`
2. 使用标准模板（见下方）
3. 更新本索引文件
4. 在 PR 中说明决策背景

## ADR 模板

```markdown
# ADR-XXX: <决策主题>

## 元信息

- 创建：YYYY-MM-DD
- 最近一次更新：YYYY-MM-DD
- 状态：draft/active/deprecated
- 版本：v1

## 背景

<!-- 为什么需要这个决策？触发原因是什么？ -->

## 决策

<!-- 最终选择的方案是什么？ -->

## 备选方案

<!-- 考虑过哪些其他方案？为什么没选？ -->

## 后果

<!-- 这个决策会带来什么影响？正面和负面的都要列出 -->

## 跟进

<!-- 需要做什么来验证这个决策？ -->

## 变更记录

- v1 - YYYY-MM-DD：初始决策
```
