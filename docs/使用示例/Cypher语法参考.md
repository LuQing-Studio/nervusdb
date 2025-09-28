# Cypher 语法参考

## 支持概览

| 特性                            | 状态         |
| ------------------------------- | ------------ |
| `MATCH (a)-[r]->(b)`            | ✅           |
| 标签过滤 `(n:Label)`            | ✅           |
| 属性过滤 `WHERE r.weight > 0.5` | ✅           |
| 路径长度 `[*1..3]`              | ✅           |
| 变量路径赋值 `p = (...)`        | ✅           |
| 聚合 `RETURN count(*)`          | ✅           |
| `ORDER BY` / `LIMIT`            | ✅           |
| `OPTIONAL MATCH`                | ⬜（规划中） |
| `UNION`                         | ✅           |
| 子查询 `CALL { ... }`           | ⬜           |

## 基本查询

```cypher
MATCH (u:Person)-[:FRIEND_OF]->(v:Person)
WHERE u.city = 'Shanghai'
RETURN v LIMIT 10;
```

## 变长路径

```cypher
MATCH p = (a:Person { id: 'user:alice' })-[:FRIEND_OF*1..4]->(b:Person)
RETURN p;
```

## 聚合

```cypher
MATCH (:Person)-[r:FRIEND_OF]->(b:Person)
RETURN b.dept AS dept, count(*) AS friendCount
ORDER BY friendCount DESC
LIMIT 5;
```

## 组合查询

```cypher
MATCH (p:Person { id: 'user:alice' })-[:FRIEND_OF]->(f)
UNION
MATCH (p:Person { id: 'user:alice' })-[:WORKS_WITH]->(f)
RETURN DISTINCT f;
```

## 注意事项

- 属性名称区分大小写
- 标签用 `labels` 属性存储，常见写法：`WHERE 'Person' IN labels(b)`
- 返回值为 JSON，字段名与 RETURN 列一致

## 故障排查

| 症状       | 解决                                                 |
| ---------- | ---------------------------------------------------- |
| 报语法错误 | 检查逗号、括号、大小写；目前不支持的语法会在日志提示 |
| 结果为空   | 确认标签、属性是否存在；可用 QueryBuilder 验证       |
| 变长路径慢 | 限制长度或增加条件过滤                               |

## 延伸阅读

- [docs/教学文档/教程-03-查询与链式联想.md](../教学文档/教程-03-查询与链式联想.md)
- [docs/使用示例/03-查询与联想-示例.md](03-查询与联想-示例.md)
