# Cypher Support in NervusDB v2

> **Status**: Production Ready (v2.0)
> **Compliance Target**: strict subset of openCypher v9
> **Reference Spec**: `docs/specs/cypher_compatibility_v2.md`

NervusDB v2 offers a robust Cypher implementation focusing on **correctness** and **embeddability**.
We follow a "Fail Fast" philosophy: features are either fully supported (passing TCK tests) or explicitly rejected with a clear error message.

## Supported Features

### Read Operations

- **Pattern Matching**:
  - `MATCH (n:Label {prop: val})`
  - Directed/Undirected relationships: `(a)-[]->(b)`, `(a)-[]-(b)`, `(a)<-[]-(b)`
  - Variable-length paths: `(a)-[*1..5]->(b)`
- **Filtering**: `WHERE` with full expression support (AND/OR/NOT/XOR, comparisons, IS NULL, IN, etc.)
- **Projection**: `RETURN` with arithmetic, functions, aliasing (`AS`), and distinct (`DISTINCT`).
- **Pipeline**:
  - `WITH`: Variable scoping, filtering, and aggregation.
  - `UNWIND`: List expansion.
  - `UNION [ALL]`: Combining result sets.
  - `ORDER BY`, `SKIP`, `LIMIT`.
- **Joins**:
  - `OPTIONAL MATCH` (Left Outer Join)
  - Cartesian Products (multiple MATCH clauses)

### Write Operations

- **Creation**: `CREATE` nodes and relationships.
- **Update**: `SET` properties (`n.prop = val`) and labels (`n:Label`).
- **Deletion**:
  - `DELETE` (nodes/rels)
  - `DETACH DELETE` (nodes with relationships)
  - `REMOVE` (properties/labels)
- **Merge**: `MERGE` with `ON CREATE` / `ON MATCH` actions.

### Subqueries & Procedures

- **Subqueries**: `CALL { ... }` (Importing variables supported).
- **Procedures**: `CALL proc.name(...) YIELD ...`
- **Existence**: `EXISTS { MATCH ... }`

### Expressions & Functions

- **Literals**: Integer, Float, String, Boolean, Null, List `[]`, Map `{}`.
- **Math**: `+`, `-`, `*`, `/`, `%`, `^`.
- **String**: `STARTS WITH`, `ENDS WITH`, `CONTAINS`.
- **Functions**:
  - Scalar: `id()`, `type()`, `labels()`, `head()`, `last()`, `size()`, `coalesce()`.
  - Aggregation: `count()`, `sum()`, `avg()`, `min()`, `max()`, `collect()`.

## Output Model

Query results are strictly typed.

- **CLI (`nervusdb-cli`)**: Outputs JSON objects.
  - Node: `{"type": "node", "id": 1, "labels": ["A"], "properties": {...}}`
  - Relationship: `{"type": "relationship", "src": 1, "rel": 2, "dst": 3, "properties": {...}}`
- **Python (`nervusdb`)**: Returns `nervusdb.Node`, `nervusdb.Relationship`, `nervusdb.Path` objects.

## Known Limitations

Please refer to `docs/specs/cypher_compatibility_v2.md` for the authoritative exclusion list.
Key omissions include:

- Regular Expressions (`=~`)
- List/Pattern Comprehensions
- Legacy Index Hints
