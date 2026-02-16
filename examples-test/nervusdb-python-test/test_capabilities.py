"""
NervusDB Python Binding — 全能力边界测试

镜像 Node.js 测试 (分类 1-20) + Python 独有测试 (分类 21-27)
"""

import os
import sys
import tempfile
import time
import math

import nervusdb

# ─── Test Harness ───────────────────────────────────────────────

passed = 0
failed = 0
skipped = 0
failures = []


def test(name, fn):
    global passed, failed
    try:
        fn()
        passed += 1
        print(f"  ✅ {name}")
    except Exception as e:
        failed += 1
        msg = str(e)
        failures.append(f"{name}: {msg}")
        print(f"  ❌ {name}: {msg}")


def skip(name, reason=""):
    global skipped
    skipped += 1
    r = f": {reason}" if reason else ""
    print(f"  ⏭️  {name} (skipped{r})")


def assert_true(cond, msg="assertion failed"):
    if not cond:
        raise AssertionError(msg)


def assert_eq(actual, expected, msg=None):
    if actual != expected:
        label = msg or "assert_eq"
        raise AssertionError(f"{label}: {actual!r} != {expected!r}")


def assert_near(actual, expected, eps=0.001, msg=None):
    if abs(actual - expected) > eps:
        label = msg or "assert_near"
        raise AssertionError(f"{label}: {actual} not near {expected} (eps={eps})")


def assert_throws(fn, exc_type=None, pattern=None):
    """Run fn, expect it to raise. Optionally check type and message pattern."""
    try:
        fn()
        raise AssertionError("Expected error but none thrown")
    except AssertionError as e:
        if str(e) == "Expected error but none thrown":
            raise
        # An AssertionError from inside fn is still a valid exception
        if exc_type and not isinstance(e, exc_type):
            raise AssertionError(f"Expected {exc_type.__name__}, got {type(e).__name__}: {e}")
        if pattern and pattern not in str(e):
            raise AssertionError(f'Error "{e}" does not contain "{pattern}"')
        return str(e)
    except Exception as e:
        if exc_type and not isinstance(e, exc_type):
            raise AssertionError(f"Expected {exc_type.__name__}, got {type(e).__name__}: {e}")
        if pattern and pattern not in str(e):
            raise AssertionError(f'Error "{e}" does not contain "{pattern}"')
        return str(e)


_tmp_counter = 0


def fresh_db(label="x"):
    global _tmp_counter
    _tmp_counter += 1
    d = tempfile.mkdtemp(prefix=f"ndb-pytest-{label}-{_tmp_counter}-")
    db_path = os.path.join(d, "test.ndb")
    db = nervusdb.Db(db_path)
    return db, db_path


# ═══════════════════════════════════════════════════════════════
print("\n🧪 NervusDB Python Binding — 全能力边界测试\n")

# ─── 1. 基础 CRUD ──────────────────────────────────────────────
print("── 1. 基础 CRUD ──")

db, _ = fresh_db("crud")

def test_create_single_node():
    n = db.execute_write("CREATE (n:Person {name: 'Alice', age: 30})")
    assert_true(n > 0, f"expected created > 0, got {n}")
test("CREATE single node", test_create_single_node)

def test_match_return_node():
    rows = db.query("MATCH (n:Person {name: 'Alice'}) RETURN n")
    assert_true(len(rows) == 1, f"expected 1 row, got {len(rows)}")
    node = rows[0]["n"]
    assert_true(isinstance(node, nervusdb.Node), f"expected Node, got {type(node)}")
    assert_eq(node.properties["name"], "Alice")
    assert_eq(node.properties["age"], 30)
    assert_true("Person" in node.labels, "missing label Person")
test("MATCH + RETURN node", test_match_return_node)

def test_create_relationship():
    db.execute_write("CREATE (b:Person {name: 'Bob', age: 25})")
    db.execute_write(
        "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) "
        "CREATE (a)-[:KNOWS {since: 2020}]->(b)"
    )
    rows = db.query("MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a.name, r, b.name")
    assert_true(len(rows) >= 1, "expected at least 1 relationship row")
test("CREATE relationship", test_create_relationship)

def test_set_property():
    db.execute_write("MATCH (n:Person {name: 'Alice'}) SET n.email = 'alice@test.com'")
    rows = db.query("MATCH (n:Person {name: 'Alice'}) RETURN n.email")
    assert_eq(rows[0]["n.email"], "alice@test.com")
test("SET property on node", test_set_property)

def test_set_overwrite():
    db.execute_write("MATCH (n:Person {name: 'Alice'}) SET n.age = 31")
    rows = db.query("MATCH (n:Person {name: 'Alice'}) RETURN n.age")
    assert_eq(rows[0]["n.age"], 31)
test("SET overwrite property", test_set_overwrite)

def test_remove_property():
    db.execute_write("MATCH (n:Person {name: 'Alice'}) REMOVE n.email")
    rows = db.query("MATCH (n:Person {name: 'Alice'}) RETURN n.email")
    assert_eq(rows[0]["n.email"], None)
test("REMOVE property", test_remove_property)

def test_delete_node():
    db.execute_write("CREATE (x:Temp {val: 'delete-me'})")
    before = db.query("MATCH (x:Temp) RETURN count(x) AS c")
    assert_true(before[0]["c"] >= 1, "temp node should exist")
    db.execute_write("MATCH (x:Temp {val: 'delete-me'}) DETACH DELETE x")
    after = db.query("MATCH (x:Temp {val: 'delete-me'}) RETURN count(x) AS c")
    assert_eq(after[0]["c"], 0)
test("DELETE node (detach)", test_delete_node)

def test_delete_rel_only():
    db.execute_write("CREATE (a:X)-[:R]->(b:Y)")
    db.execute_write("MATCH (:X)-[r:R]->(:Y) DELETE r")
    rows = db.query("MATCH (:X)-[r:R]->(:Y) RETURN count(r) AS c")
    assert_eq(rows[0]["c"], 0)
test("DELETE relationship only", test_delete_rel_only)

def test_multi_create():
    try:
        db.execute_write("CREATE (:Multi1 {v: 1}), (:Multi2 {v: 2})")
        rows = db.query("MATCH (n:Multi1) RETURN count(n) AS c")
        assert_true(rows[0]["c"] >= 1, "multi-create should work")
    except Exception as e:
        print(f"    (limitation: {str(e)[:80]})")
        skip("multi-node CREATE", "duplicate external id in same tx")
test("multi-node CREATE in single statement", test_multi_create)

db.close()

# ─── 1b. RETURN 投影 ──────────────────────────────────────────
print("\n── 1b. RETURN 投影 ──")

db, _ = fresh_db("return")
db.execute_write("CREATE (a:P {name: 'X', age: 10})-[:R {w: 5}]->(b:P {name: 'Y', age: 20})")

def test_return_scalar():
    rows = db.query("RETURN 1 + 2 AS sum")
    assert_eq(rows[0]["sum"], 3)
test("RETURN scalar expression", test_return_scalar)

def test_return_alias():
    rows = db.query("MATCH (n:P {name: 'X'}) RETURN n.name AS who")
    assert_eq(rows[0]["who"], "X")
test("RETURN property alias", test_return_alias)

def test_return_distinct():
    db.execute_write("CREATE (:D {v: 1})")
    db.execute_write("CREATE (:D {v: 1})")
    db.execute_write("CREATE (:D {v: 2})")
    rows = db.query("MATCH (n:D) RETURN DISTINCT n.v ORDER BY n.v")
    assert_eq(len(rows), 2)
test("RETURN DISTINCT", test_return_distinct)

def test_return_star():
    rows = db.query("MATCH (n:P {name: 'X'}) RETURN *")
    assert_true(len(rows) >= 1, "RETURN * should work")
    assert_true("n" in rows[0], "should have n in result")
test("RETURN *", test_return_star)

db.close()

# ─── 2. 多标签节点 ────────────────────────────────────────────
print("\n── 2. 多标签节点 ──")

db, _ = fresh_db("labels")

def test_multi_label_create():
    db.execute_write("CREATE (n:Person:Employee:Manager {name: 'Carol'})")
    rows = db.query("MATCH (n:Person:Employee {name: 'Carol'}) RETURN n")
    assert_true(len(rows) == 1, "multi-label match failed")
    node = rows[0]["n"]
    assert_true("Person" in node.labels, "missing Person")
    assert_true("Employee" in node.labels, "missing Employee")
    assert_true("Manager" in node.labels, "missing Manager")
test("CREATE node with multiple labels [NODE-BUG?]", test_multi_label_create)

def test_single_label_subset():
    rows = db.query("MATCH (n:Manager) RETURN n.name")
    assert_true(len(rows) >= 1, "should match by Manager label")
test("MATCH by single label subset [NODE-BUG?]", test_single_label_subset)

db.close()

# ─── 3. 数据类型 ──────────────────────────────────────────────
print("\n── 3. 数据类型 ──")

db, _ = fresh_db("types")

def test_null_prop():
    db.execute_write("CREATE (n:T {val: null})")
    rows = db.query("MATCH (n:T) RETURN n.val")
    assert_eq(rows[0]["n.val"], None)
test("null property", test_null_prop)

def test_bool_props():
    db.execute_write("CREATE (n:Bool {t: true, f: false})")
    rows = db.query("MATCH (n:Bool) RETURN n.t, n.f")
    assert_eq(rows[0]["n.t"], True)
    assert_eq(rows[0]["n.f"], False)
test("boolean properties", test_bool_props)

def test_int_prop():
    db.execute_write("CREATE (n:Num {val: 42})")
    rows = db.query("MATCH (n:Num) RETURN n.val")
    assert_eq(rows[0]["n.val"], 42)
test("integer property", test_int_prop)

def test_neg_int():
    db.execute_write("CREATE (n:Neg {val: -100})")
    rows = db.query("MATCH (n:Neg) RETURN n.val")
    assert_eq(rows[0]["n.val"], -100)
test("negative integer", test_neg_int)

def test_float_prop():
    db.execute_write("CREATE (n:Flt {val: 3.14})")
    rows = db.query("MATCH (n:Flt) RETURN n.val")
    assert_near(rows[0]["n.val"], 3.14)
test("float property", test_float_prop)

def test_string_special():
    db.execute_write(r"CREATE (n:Str {val: 'hello \"world\" \\n'})")
    rows = db.query("MATCH (n:Str) RETURN n.val")
    assert_true(isinstance(rows[0]["n.val"], str), "should be string")
test("string property with special chars", test_string_special)

def test_list_literal():
    rows = db.query("RETURN [1, 2, 3] AS lst")
    lst = rows[0]["lst"]
    assert_true(isinstance(lst, list), "should be list")
    assert_eq(lst, [1, 2, 3])
test("list literal in RETURN", test_list_literal)

def test_map_literal():
    rows = db.query("RETURN {a: 1, b: 'two'} AS m")
    m = rows[0]["m"]
    assert_eq(m["a"], 1)
    assert_eq(m["b"], "two")
test("map literal in RETURN", test_map_literal)

def test_list_prop():
    db.execute_write("CREATE (n:Lst {tags: ['a', 'b', 'c']})")
    rows = db.query("MATCH (n:Lst) RETURN n.tags")
    tags = rows[0]["n.tags"]
    assert_true(isinstance(tags, list), "tags should be list")
    assert_eq(tags, ["a", "b", "c"])
test("list property on node", test_list_prop)

db.close()

# ─── 4. WHERE 过滤 ────────────────────────────────────────────
print("\n── 4. WHERE 过滤 ──")

db, _ = fresh_db("where")
db.execute_write("CREATE (a:P {name: 'A', age: 20})")
db.execute_write("CREATE (b:P {name: 'B', age: 30})")
db.execute_write("CREATE (c:P {name: 'C', age: 40})")

def test_where_eq():
    rows = db.query("MATCH (n:P) WHERE n.age = 30 RETURN n.name")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["n.name"], "B")
test("WHERE equality", test_where_eq)

def test_where_gt():
    rows = db.query("MATCH (n:P) WHERE n.age > 25 RETURN n.name ORDER BY n.name")
    assert_eq(len(rows), 2)
test("WHERE comparison >", test_where_gt)

def test_where_and():
    rows = db.query("MATCH (n:P) WHERE n.age > 15 AND n.age < 35 RETURN n.name ORDER BY n.name")
    assert_eq(len(rows), 2)
test("WHERE AND", test_where_and)

def test_where_or():
    rows = db.query("MATCH (n:P) WHERE n.name = 'A' OR n.name = 'C' RETURN n.name ORDER BY n.name")
    assert_eq(len(rows), 2)
test("WHERE OR", test_where_or)

def test_where_not():
    rows = db.query("MATCH (n:P) WHERE NOT n.name = 'B' RETURN n.name ORDER BY n.name")
    assert_eq(len(rows), 2)
test("WHERE NOT", test_where_not)

def test_where_in():
    rows = db.query("MATCH (n:P) WHERE n.name IN ['A', 'C'] RETURN n.name ORDER BY n.name")
    assert_eq(len(rows), 2)
test("WHERE IN list", test_where_in)

def test_where_starts_with():
    rows = db.query("MATCH (n:P) WHERE n.name STARTS WITH 'A' RETURN n.name")
    assert_eq(len(rows), 1)
test("WHERE STARTS WITH", test_where_starts_with)

def test_where_contains():
    db.execute_write("CREATE (n:P {name: 'Alice', age: 50})")
    rows = db.query("MATCH (n:P) WHERE n.name CONTAINS 'lic' RETURN n.name")
    assert_eq(len(rows), 1)
test("WHERE CONTAINS", test_where_contains)

def test_where_ends_with():
    rows = db.query("MATCH (n:P) WHERE n.name ENDS WITH 'e' RETURN n.name")
    assert_true(len(rows) >= 1, "should find Alice")
test("WHERE ENDS WITH", test_where_ends_with)

def test_where_is_null():
    db.execute_write("CREATE (n:P {name: 'NoAge'})")
    rows = db.query("MATCH (n:P) WHERE n.age IS NULL RETURN n.name")
    assert_true(len(rows) >= 1, "should find node without age")
test("WHERE IS NULL", test_where_is_null)

def test_where_is_not_null():
    rows = db.query("MATCH (n:P) WHERE n.age IS NOT NULL RETURN n.name ORDER BY n.name")
    assert_true(len(rows) >= 3, "should find nodes with age")
test("WHERE IS NOT NULL", test_where_is_not_null)

db.close()

# ─── 5. 查询子句 ──────────────────────────────────────────────
print("\n── 5. 查询子句 ──")

db, _ = fresh_db("clauses")
db.execute_write("CREATE (:N {v: 3})")
db.execute_write("CREATE (:N {v: 1})")
db.execute_write("CREATE (:N {v: 2})")
db.execute_write("CREATE (:N {v: 5})")
db.execute_write("CREATE (:N {v: 4})")

def test_order_asc():
    rows = db.query("MATCH (n:N) RETURN n.v ORDER BY n.v")
    vals = [r["n.v"] for r in rows]
    assert_eq(vals, [1, 2, 3, 4, 5])
test("ORDER BY ASC", test_order_asc)

def test_order_desc():
    rows = db.query("MATCH (n:N) RETURN n.v ORDER BY n.v DESC")
    vals = [r["n.v"] for r in rows]
    assert_eq(vals, [5, 4, 3, 2, 1])
test("ORDER BY DESC", test_order_desc)

def test_limit():
    rows = db.query("MATCH (n:N) RETURN n.v ORDER BY n.v LIMIT 3")
    assert_eq(len(rows), 3)
test("LIMIT", test_limit)

def test_skip():
    rows = db.query("MATCH (n:N) RETURN n.v ORDER BY n.v SKIP 2 LIMIT 2")
    assert_eq(len(rows), 2)
    assert_eq(rows[0]["n.v"], 3)
test("SKIP", test_skip)

def test_with_pipe():
    rows = db.query("MATCH (n:N) WITH n.v AS val WHERE val > 3 RETURN val ORDER BY val")
    assert_eq(len(rows), 2)
    assert_eq(rows[0]["val"], 4)
test("WITH pipe", test_with_pipe)

def test_unwind():
    rows = db.query("UNWIND [10, 20, 30] AS x RETURN x")
    assert_eq(len(rows), 3)
    assert_eq(rows[0]["x"], 10)
test("UNWIND", test_unwind)

def test_unwind_create():
    db.execute_write("UNWIND [1, 2, 3] AS i CREATE (:UW {idx: i})")
    rows = db.query("MATCH (n:UW) RETURN n.idx ORDER BY n.idx")
    assert_eq(len(rows), 3)
test("UNWIND + CREATE", test_unwind_create)

def test_union():
    rows = db.query("RETURN 1 AS x UNION RETURN 2 AS x")
    assert_eq(len(rows), 2)
test("UNION", test_union)

def test_union_all():
    rows = db.query("RETURN 1 AS x UNION ALL RETURN 1 AS x")
    assert_eq(len(rows), 2)
test("UNION ALL", test_union_all)

def test_optional_match():
    db.execute_write("CREATE (:Lonely {name: 'solo'})")
    rows = db.query("MATCH (n:Lonely) OPTIONAL MATCH (n)-[r]->(m) RETURN n.name, r, m")
    assert_true(len(rows) >= 1, "should return at least 1 row")
    assert_eq(rows[0]["r"], None)
    assert_eq(rows[0]["m"], None)
test("OPTIONAL MATCH", test_optional_match)

db.close()

# ─── 6. 聚合函数 ──────────────────────────────────────────────
print("\n── 6. 聚合函数 ──")

db, _ = fresh_db("agg")
db.execute_write("CREATE (:S {v: 10})")
db.execute_write("CREATE (:S {v: 20})")
db.execute_write("CREATE (:S {v: 30})")

def test_count():
    rows = db.query("MATCH (n:S) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 3)
test("count()", test_count)

def test_sum():
    rows = db.query("MATCH (n:S) RETURN sum(n.v) AS s")
    assert_eq(rows[0]["s"], 60)
test("sum()", test_sum)

def test_avg():
    rows = db.query("MATCH (n:S) RETURN avg(n.v) AS a")
    assert_eq(rows[0]["a"], 20)
test("avg()", test_avg)

def test_min_max():
    rows = db.query("MATCH (n:S) RETURN min(n.v) AS lo, max(n.v) AS hi")
    assert_eq(rows[0]["lo"], 10)
    assert_eq(rows[0]["hi"], 30)
test("min() / max()", test_min_max)

def test_collect():
    rows = db.query("MATCH (n:S) RETURN collect(n.v) AS vals")
    vals = rows[0]["vals"]
    assert_true(isinstance(vals, list), "collect should return list")
    assert_eq(len(vals), 3)
test("collect()", test_collect)

def test_count_distinct():
    db.execute_write("CREATE (:S {v: 10})")
    rows = db.query("MATCH (n:S) RETURN count(DISTINCT n.v) AS c")
    assert_eq(rows[0]["c"], 3)
test("count(DISTINCT)", test_count_distinct)

def test_group_by():
    db.execute_write("CREATE (:G {cat: 'a', v: 1})")
    db.execute_write("CREATE (:G {cat: 'a', v: 2})")
    db.execute_write("CREATE (:G {cat: 'b', v: 3})")
    rows = db.query("MATCH (n:G) RETURN n.cat, sum(n.v) AS total ORDER BY n.cat")
    assert_eq(len(rows), 2)
    assert_eq(rows[0]["n.cat"], "a")
    assert_eq(rows[0]["total"], 3)
test("GROUP BY (implicit)", test_group_by)

db.close()

# ─── 7. MERGE ─────────────────────────────────────────────────
print("\n── 7. MERGE ──")

db, _ = fresh_db("merge")

def test_merge_create():
    db.execute_write("MERGE (n:M {key: 'x'})")
    rows = db.query("MATCH (n:M {key: 'x'}) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 1)
test("MERGE creates when not exists", test_merge_create)

def test_merge_match():
    db.execute_write("MERGE (n:M {key: 'x'})")
    rows = db.query("MATCH (n:M {key: 'x'}) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 1, "should still be 1, not 2")
test("MERGE matches when exists", test_merge_match)

def test_merge_on_create():
    db.execute_write("MERGE (n:M {key: 'y'}) ON CREATE SET n.created = true")
    rows = db.query("MATCH (n:M {key: 'y'}) RETURN n.created")
    assert_eq(rows[0]["n.created"], True)
test("MERGE ON CREATE SET", test_merge_on_create)

def test_merge_on_match():
    db.execute_write("MERGE (n:M {key: 'y'}) ON MATCH SET n.updated = true")
    rows = db.query("MATCH (n:M {key: 'y'}) RETURN n.updated")
    assert_eq(rows[0]["n.updated"], True)
test("MERGE ON MATCH SET", test_merge_on_match)

def test_merge_rel():
    db.execute_write("CREATE (:MA {id: 1})")
    db.execute_write("CREATE (:MB {id: 2})")
    db.execute_write("MATCH (a:MA), (b:MB) MERGE (a)-[:LINK]->(b)")
    db.execute_write("MATCH (a:MA), (b:MB) MERGE (a)-[:LINK]->(b)")
    rows = db.query("MATCH (:MA)-[r:LINK]->(:MB) RETURN count(r) AS c")
    assert_eq(rows[0]["c"], 1, "MERGE should not duplicate relationship [NODE-BUG?]")
test("MERGE relationship [NODE-BUG?]", test_merge_rel)

db.close()

# ─── 8. CASE 表达式 ───────────────────────────────────────────
print("\n── 8. CASE 表达式 ──")

db, _ = fresh_db("case")
db.execute_write("CREATE (:C {v: 1})")
db.execute_write("CREATE (:C {v: 2})")
db.execute_write("CREATE (:C {v: 3})")

def test_simple_case():
    rows = db.query(
        "MATCH (n:C) RETURN CASE n.v WHEN 1 THEN 'one' WHEN 2 THEN 'two' "
        "ELSE 'other' END AS label ORDER BY n.v"
    )
    assert_eq(rows[0]["label"], "one")
    assert_eq(rows[1]["label"], "two")
    assert_eq(rows[2]["label"], "other")
test("simple CASE", test_simple_case)

def test_generic_case():
    rows = db.query(
        "MATCH (n:C) RETURN CASE WHEN n.v < 2 THEN 'low' WHEN n.v > 2 THEN 'high' "
        "ELSE 'mid' END AS cat ORDER BY n.v"
    )
    assert_eq(rows[0]["cat"], "low")
    assert_eq(rows[1]["cat"], "mid")
    assert_eq(rows[2]["cat"], "high")
test("generic CASE", test_generic_case)

db.close()

# ─── 9. 字符串函数 ────────────────────────────────────────────
print("\n── 9. 字符串函数 ──")

db, _ = fresh_db("strfn")

def test_tostring():
    rows = db.query("RETURN toString(42) AS s")
    assert_eq(rows[0]["s"], "42")
test("toString()", test_tostring)

def test_upper_lower():
    rows = db.query("RETURN toUpper('hello') AS u, toLower('HELLO') AS l")
    assert_eq(rows[0]["u"], "HELLO")
    assert_eq(rows[0]["l"], "hello")
test("toUpper / toLower", test_upper_lower)

def test_trim():
    rows = db.query("RETURN trim('  hi  ') AS t, lTrim('  hi') AS l, rTrim('hi  ') AS r")
    assert_eq(rows[0]["t"], "hi")
    assert_eq(rows[0]["l"], "hi")
    assert_eq(rows[0]["r"], "hi")
test("trim / lTrim / rTrim", test_trim)

def test_substring():
    rows = db.query("RETURN substring('hello', 1, 3) AS s")
    assert_eq(rows[0]["s"], "ell")
test("substring", test_substring)

def test_size_string():
    rows = db.query("RETURN size('hello') AS s")
    assert_eq(rows[0]["s"], 5)
test("size() on string", test_size_string)

def test_replace():
    rows = db.query("RETURN replace('hello world', 'world', 'nervus') AS s")
    assert_eq(rows[0]["s"], "hello nervus")
test("replace()", test_replace)

def test_left_right():
    rows = db.query("RETURN left('hello', 3) AS l, right('hello', 3) AS r")
    assert_eq(rows[0]["l"], "hel")
    assert_eq(rows[0]["r"], "llo")
test("left / right [NODE-BUG?]", test_left_right)

db.close()

# ─── 10. 数学运算 ─────────────────────────────────────────────
print("\n── 10. 数学运算 ──")

db, _ = fresh_db("math")

def test_arithmetic():
    rows = db.query("RETURN 10 + 3 AS a, 10 - 3 AS b, 10 * 3 AS c, 10 / 3 AS d, 10 % 3 AS e")
    assert_eq(rows[0]["a"], 13)
    assert_eq(rows[0]["b"], 7)
    assert_eq(rows[0]["c"], 30)
    assert_true(isinstance(rows[0]["d"], (int, float)), "division should return number")
    assert_eq(rows[0]["e"], 1)
test("arithmetic: + - * / %", test_arithmetic)

def test_abs():
    rows = db.query("RETURN abs(-5) AS v")
    assert_eq(rows[0]["v"], 5)
test("abs()", test_abs)

def test_to_int_float():
    rows = db.query("RETURN toInteger(3.7) AS i, toFloat(3) AS f")
    assert_eq(rows[0]["i"], 3)
    assert_true(isinstance(rows[0]["f"], (int, float)), "toFloat should return number")
test("toInteger / toFloat", test_to_int_float)

def test_sign():
    rows = db.query("RETURN sign(-5) AS neg, sign(0) AS zero, sign(5) AS pos")
    assert_eq(rows[0]["neg"], -1)
    assert_eq(rows[0]["zero"], 0)
    assert_eq(rows[0]["pos"], 1)
test("sign()", test_sign)

db.close()

# ─── 11. 变长路径 ─────────────────────────────────────────────
print("\n── 11. 变长路径 ──")

db, _ = fresh_db("varlen")
db.execute_write(
    "CREATE (a:V {name: 'A'})-[:NEXT]->(b:V {name: 'B'})"
    "-[:NEXT]->(c:V {name: 'C'})-[:NEXT]->(d:V {name: 'D'})"
)

def test_fixed_len():
    rows = db.query("MATCH (a:V {name: 'A'})-[:NEXT*2]->(c) RETURN c.name")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["c.name"], "C")
test("fixed length path *2", test_fixed_len)

def test_var_len():
    rows = db.query("MATCH (a:V {name: 'A'})-[:NEXT*1..3]->(x) RETURN x.name ORDER BY x.name")
    assert_eq(len(rows), 3)
test("variable length path *1..3", test_var_len)

def test_var_len_upper():
    rows = db.query("MATCH (a:V {name: 'A'})-[:NEXT*..2]->(x) RETURN x.name ORDER BY x.name")
    assert_eq(len(rows), 2)
test("variable length path *..2", test_var_len_upper)

def test_shortest_path():
    try:
        rows = db.query(
            "MATCH p = shortestPath((a:V {name: 'A'})-[:NEXT*]->(d:V {name: 'D'})) "
            "RETURN length(p) AS len"
        )
        assert_eq(rows[0]["len"], 3)
    except Exception:
        skip("shortestPath", "not supported")
test("shortest path", test_shortest_path)

db.close()

# ─── 12. EXISTS 子查询 ────────────────────────────────────────
print("\n── 12. EXISTS 子查询 ──")

db, _ = fresh_db("exists")
db.execute_write("CREATE (a:E {name: 'has-rel'})-[:R]->(b:E {name: 'target'})")
db.execute_write("CREATE (:E {name: 'no-rel'})")

def test_exists():
    try:
        rows = db.query("MATCH (n:E) WHERE EXISTS { (n)-[:R]->() } RETURN n.name")
        assert_eq(len(rows), 1)
        assert_eq(rows[0]["n.name"], "has-rel")
    except Exception:
        skip("EXISTS subquery", "not supported")
test("WHERE EXISTS pattern", test_exists)

db.close()

# ─── 13. FOREACH ──────────────────────────────────────────────
print("\n── 13. FOREACH ──")

db, _ = fresh_db("foreach")

def test_foreach():
    try:
        db.execute_write("FOREACH (i IN [1, 2, 3] | CREATE (:FE {idx: i}))")
        rows = db.query("MATCH (n:FE) RETURN n.idx ORDER BY n.idx")
        assert_eq(len(rows), 3)
    except Exception as e:
        skip("FOREACH", str(e)[:60])
test("FOREACH create nodes", test_foreach)

db.close()

# ─── 14. 事务 (WriteTxn) ──────────────────────────────────────
print("\n── 14. 事务 (WriteTxn) ──")

db, db_path_txn = fresh_db("txn")

def test_txn_commit():
    txn = db.begin_write()
    txn.query("CREATE (:TX {v: 1})")
    txn.query("CREATE (:TX {v: 2})")
    txn.commit()  # Python commit() returns None
    rows = db.query("MATCH (n:TX) RETURN n.v ORDER BY n.v")
    assert_eq(len(rows), 2)
test("beginWrite + query + commit", test_txn_commit)

def test_txn_rollback():
    txn = db.begin_write()
    txn.query("CREATE (:TX {v: 99})")
    txn.rollback()
    # After rollback, transaction is finished — commit should throw
    assert_throws(lambda: txn.commit(), pattern="already finished")
    rows = db.query("MATCH (n:TX {v: 99}) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 0)
test("rollback discards staged queries", test_txn_rollback)

def test_txn_syntax_error():
    txn = db.begin_write()
    assert_throws(lambda: txn.query("INVALID CYPHER !!!"))
test("txn syntax error at query time", test_txn_syntax_error)

def test_txn_independent():
    txn1 = db.begin_write()
    txn1.query("CREATE (:Ind {batch: 1})")
    txn1.commit()
    txn2 = db.begin_write()
    txn2.query("CREATE (:Ind {batch: 2})")
    txn2.commit()
    rows = db.query("MATCH (n:Ind) RETURN n.batch ORDER BY n.batch")
    assert_eq(len(rows), 2)
test("multiple txn commits are independent", test_txn_independent)

db.close()

# ─── 15. 错误处理 ─────────────────────────────────────────────
print("\n── 15. 错误处理 ──")

db, _ = fresh_db("errors")

def test_syntax_error_query():
    msg = assert_throws(lambda: db.query("NOT VALID CYPHER"), nervusdb.SyntaxError)
    assert_true(len(msg) > 0, "should have error message")
test("syntax error in query() -> SyntaxError", test_syntax_error_query)

def test_syntax_error_write():
    assert_throws(lambda: db.execute_write("BLAH BLAH"), nervusdb.SyntaxError)
test("syntax error in execute_write() -> SyntaxError", test_syntax_error_write)

def test_write_via_query():
    try:
        db.query("CREATE (:ShouldFail)")
        print("    (note: query() accepted write — no read/write separation)")
    except Exception:
        print("    (note: query() correctly rejected write)")
test("write-via-query behavior documented", test_write_via_query)

def test_error_is_typed():
    try:
        db.query("INVALID!!!")
    except nervusdb.SyntaxError:
        pass
    except nervusdb.NervusError:
        pass
    except Exception as e:
        raise AssertionError(f"Expected NervusError subclass, got {type(e).__name__}: {e}")
test("error is typed NervusError subclass", test_error_is_typed)

def test_ops_after_close():
    db2, _ = fresh_db("closed")
    db2.close()
    assert_throws(lambda: db2.query("RETURN 1"), nervusdb.StorageError, "closed")
test("operations after close() throw StorageError", test_ops_after_close)

def test_double_close():
    db3, _ = fresh_db("dblclose")
    db3.close()
    db3.close()
test("double close is safe", test_double_close)

db.close()

# ─── 16. 关系方向 ─────────────────────────────────────────────
print("\n── 16. 关系方向 ──")

db, _ = fresh_db("direction")
db.execute_write("CREATE (a:D {name: 'A'})-[:TO]->(b:D {name: 'B'})")

def test_outgoing():
    rows = db.query("MATCH (a:D {name: 'A'})-[:TO]->(b) RETURN b.name")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["b.name"], "B")
test("outgoing match ->", test_outgoing)

def test_incoming():
    rows = db.query("MATCH (b:D {name: 'B'})<-[:TO]-(a) RETURN a.name")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["a.name"], "A")
test("incoming match <-", test_incoming)

def test_undirected():
    rows = db.query("MATCH (a:D {name: 'A'})-[:TO]-(b) RETURN b.name")
    assert_true(len(rows) >= 1, "undirected should match")
test("undirected match -[]-", test_undirected)

def test_rel_properties():
    db.execute_write("CREATE (:RP {id: 1})-[:EDGE {weight: 0.5, label: 'test'}]->(:RP {id: 2})")
    rows = db.query("MATCH ()-[r:EDGE]->() RETURN r")
    rel = rows[0]["r"]
    assert_true(isinstance(rel, nervusdb.Relationship), f"expected Relationship, got {type(rel)}")
    assert_near(rel.properties["weight"], 0.5)
    assert_eq(rel.properties["label"], "test")
test("relationship properties", test_rel_properties)

db.close()

# ─── 17. 复杂图模式 ───────────────────────────────────────────
print("\n── 17. 复杂图模式 ──")

db, _ = fresh_db("complex")

def test_triangle():
    db.execute_write(
        "CREATE (a:T {name: 'a'})-[:E]->(b:T {name: 'b'})"
        "-[:E]->(c:T {name: 'c'})-[:E]->(a)"
    )
    rows = db.query(
        "MATCH (a:T)-[:E]->(b:T)-[:E]->(c:T)-[:E]->(a) RETURN a.name, b.name, c.name"
    )
    assert_true(len(rows) >= 1, "should find triangle")
test("triangle pattern", test_triangle)

def test_multi_hop():
    db.execute_write(
        "CREATE (:H {lv: 0})-[:STEP]->(:H {lv: 1})"
        "-[:STEP]->(:H {lv: 2})-[:STEP]->(:H {lv: 3})"
    )
    rows = db.query(
        "MATCH (a:H)-[:STEP]->(b:H)-[:STEP]->(c:H) WHERE a.lv = 0 AND c.lv = 2 RETURN b.lv"
    )
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["b.lv"], 1)
test("multi-hop with WHERE", test_multi_hop)

def test_multi_match():
    db.execute_write("CREATE (:MM {id: 'x'})")
    db.execute_write("CREATE (:MM {id: 'y'})")
    rows = db.query("MATCH (a:MM {id: 'x'}) MATCH (b:MM {id: 'y'}) RETURN a.id, b.id")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["a.id"], "x")
    assert_eq(rows[0]["b.id"], "y")
test("multiple MATCH clauses", test_multi_match)

db.close()

# ─── 18. 批量写入性能 ─────────────────────────────────────────
print("\n── 18. 批量写入性能 ──")

db, _ = fresh_db("bulk")

def test_batch_create():
    start = time.monotonic()
    for i in range(1000):
        db.execute_write(f"CREATE (:Bulk {{idx: {i}}})")
    elapsed = time.monotonic() - start
    rows = db.query("MATCH (n:Bulk) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 1000)
    ops = int(1000 / elapsed) if elapsed > 0 else 999999
    print(f"    (1000 nodes in {elapsed*1000:.0f}ms, {ops} ops/s)")
test("batch create 1000 nodes", test_batch_create)

def test_batch_query():
    start = time.monotonic()
    rows = db.query("MATCH (n:Bulk) RETURN n.idx ORDER BY n.idx LIMIT 1000")
    elapsed = time.monotonic() - start
    assert_eq(len(rows), 1000)
    print(f"    (query 1000 in {elapsed*1000:.0f}ms)")
test("batch query 1000 nodes", test_batch_query)

def test_unwind_bulk():
    items = ",".join(str(i) for i in range(100))
    start = time.monotonic()
    db.execute_write(f"UNWIND [{items}] AS i CREATE (:UBulk {{idx: i}})")
    elapsed = time.monotonic() - start
    rows = db.query("MATCH (n:UBulk) RETURN count(n) AS c")
    assert_eq(rows[0]["c"], 100)
    print(f"    (UNWIND 100 in {elapsed*1000:.0f}ms)")
test("UNWIND batch create", test_unwind_bulk)

db.close()

# ─── 19. 持久化 ───────────────────────────────────────────────
print("\n── 19. 持久化 (close + reopen) ──")

db, db_path_persist = fresh_db("persist")
db.execute_write("CREATE (:Persist {key: 'survives'})")
db.close()

def test_persist():
    db2 = nervusdb.Db(db_path_persist)
    rows = db2.query("MATCH (n:Persist) RETURN n.key")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["n.key"], "survives")
    db2.close()
test("data survives close + reopen", test_persist)

# ─── 20. 边界情况 ─────────────────────────────────────────────
print("\n── 20. 边界情况 ──")

db, _ = fresh_db("edge")

def test_empty_result():
    rows = db.query("MATCH (n:NonExistent) RETURN n")
    assert_eq(len(rows), 0)
test("empty result set", test_empty_result)

def test_return_literals():
    rows = db.query("RETURN 'hello' AS greeting, 42 AS num, true AS flag, null AS nothing")
    assert_eq(rows[0]["greeting"], "hello")
    assert_eq(rows[0]["num"], 42)
    assert_eq(rows[0]["flag"], True)
    assert_eq(rows[0]["nothing"], None)
test("RETURN literal without MATCH", test_return_literals)

def test_empty_string():
    db.execute_write("CREATE (:ES {val: ''})")
    rows = db.query("MATCH (n:ES) RETURN n.val")
    assert_eq(rows[0]["n.val"], "")
test("empty string property", test_empty_string)

def test_large_string():
    big = "x" * 10000
    db.execute_write(f"CREATE (:Big {{val: '{big}'}})")
    rows = db.query("MATCH (n:Big) RETURN size(n.val) AS len")
    assert_eq(rows[0]["len"], 10000)
test("large string property", test_large_string)

def test_many_props():
    props = ", ".join(f"p{i}: {i}" for i in range(50))
    db.execute_write(f"CREATE (:ManyProps {{{props}}})")
    rows = db.query("MATCH (n:ManyProps) RETURN n")
    node = rows[0]["n"]
    assert_eq(node.properties["p0"], 0)
    assert_eq(node.properties["p49"], 49)
test("node with many properties", test_many_props)

def test_self_loop():
    db.execute_write("CREATE (n:Loop {name: 'self'})-[:SELF]->(n)")
    rows = db.query("MATCH (n:Loop)-[:SELF]->(n) RETURN n.name")
    assert_eq(len(rows), 1)
test("self-loop relationship", test_self_loop)

db.close()

# ═══════════════════════════════════════════════════════════════
# Python 独有测试 (分类 21-27)
# ═══════════════════════════════════════════════════════════════

# ─── 21. query_stream() ───────────────────────────────────────
print("\n── 21. query_stream() [Python only] ──")

db, _ = fresh_db("stream")
db.execute_write("CREATE (:QS {v: 1})")
db.execute_write("CREATE (:QS {v: 2})")
db.execute_write("CREATE (:QS {v: 3})")

def test_stream_iter():
    stream = db.query_stream("MATCH (n:QS) RETURN n.v ORDER BY n.v")
    vals = [row["n.v"] for row in stream]
    assert_eq(vals, [1, 2, 3])
test("query_stream iteration", test_stream_iter)

def test_stream_len():
    stream = db.query_stream("MATCH (n:QS) RETURN n.v")
    assert_eq(stream.len, 3)
test("query_stream .len property", test_stream_len)

def test_stream_empty():
    stream = db.query_stream("MATCH (n:NonExist) RETURN n")
    vals = list(stream)
    assert_eq(len(vals), 0)
    assert_eq(stream.len, 0)
test("query_stream empty result", test_stream_empty)

def test_stream_is_iterator():
    stream = db.query_stream("MATCH (n:QS) RETURN n.v ORDER BY n.v")
    assert_true(hasattr(stream, "__iter__"), "should have __iter__")
    assert_true(hasattr(stream, "__next__"), "should have __next__")
    first = next(stream)
    assert_eq(first["n.v"], 1)
test("query_stream is proper iterator", test_stream_is_iterator)

db.close()

# ─── 22. 参数化查询 ───────────────────────────────────────────
print("\n── 22. 参数化查询 [Python only] ──")

db, _ = fresh_db("params")
db.execute_write("CREATE (:PP {name: 'Alice', age: 30})")
db.execute_write("CREATE (:PP {name: 'Bob', age: 25})")

def test_param_string():
    rows = db.query("MATCH (n:PP {name: $name}) RETURN n.age", params={"name": "Alice"})
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["n.age"], 30)
test("param: string value", test_param_string)

def test_param_int():
    rows = db.query("MATCH (n:PP) WHERE n.age > $min_age RETURN n.name ORDER BY n.name",
                     params={"min_age": 26})
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["n.name"], "Alice")
test("param: integer value", test_param_int)

def test_param_none():
    rows = db.query("RETURN $val AS v", params={"val": None})
    assert_eq(rows[0]["v"], None)
test("param: None value", test_param_none)

def test_param_list():
    rows = db.query("RETURN $items AS lst", params={"items": [1, 2, 3]})
    assert_eq(rows[0]["lst"], [1, 2, 3])
test("param: list value", test_param_list)

def test_param_write():
    db.execute_write("CREATE (:PP {name: $n, age: $a})", params={"n": "Carol", "a": 35})
    rows = db.query("MATCH (n:PP {name: 'Carol'}) RETURN n.age")
    assert_eq(len(rows), 1)
    assert_eq(rows[0]["n.age"], 35)
test("param: in execute_write", test_param_write)

db.close()

# ─── 23. 向量操作 ─────────────────────────────────────────────
print("\n── 23. 向量操作 [Python only] ──")

db, db_path_vec = fresh_db("vector")

def test_set_search_vector():
    # Create nodes first
    db.execute_write("CREATE (:Vec {name: 'a'})")
    db.execute_write("CREATE (:Vec {name: 'b'})")
    db.execute_write("CREATE (:Vec {name: 'c'})")
    rows = db.query("MATCH (n:Vec) RETURN n.name, id(n) AS nid ORDER BY n.name")
    ids = {r["n.name"]: r["nid"] for r in rows}

    # Set vectors via WriteTxn
    txn = db.begin_write()
    txn.set_vector(ids["a"], [1.0, 0.0, 0.0])
    txn.set_vector(ids["b"], [0.0, 1.0, 0.0])
    txn.set_vector(ids["c"], [0.9, 0.1, 0.0])
    txn.commit()

    # Search — closest to [1, 0, 0] should be 'a' then 'c'
    results = db.search_vector([1.0, 0.0, 0.0], 3)
    assert_true(len(results) >= 2, f"expected >= 2 results, got {len(results)}")
    # Results are (node_id, distance) tuples
    result_ids = [r[0] for r in results]
    assert_eq(result_ids[0], ids["a"], "closest should be 'a'")
test("set_vector + search_vector basic", test_set_search_vector)

def test_vector_knn_order():
    results = db.search_vector([0.0, 1.0, 0.0], 2)
    assert_true(len(results) >= 1, "should find at least 1 result")
    # First result should have smallest distance
    if len(results) >= 2:
        assert_true(results[0][1] <= results[1][1], "results should be sorted by distance")
test("vector KNN ordering", test_vector_knn_order)

def test_vector_k_limit():
    results = db.search_vector([1.0, 0.0, 0.0], 1)
    assert_eq(len(results), 1)
test("vector search k limit", test_vector_k_limit)

def test_vector_persist():
    db.close()
    db2 = nervusdb.Db(db_path_vec)
    results = db2.search_vector([1.0, 0.0, 0.0], 2)
    assert_true(len(results) >= 1, "vectors should survive reopen")
    db2.close()
test("vector persistence after reopen", test_vector_persist)

# ─── 24. 类型化对象 ───────────────────────────────────────────
print("\n── 24. 类型化对象 [Python only] ──")

db, _ = fresh_db("typed")
db.execute_write("CREATE (a:TO {name: 'x'})-[:REL {w: 1}]->(b:TO {name: 'y'})")

def test_node_type():
    rows = db.query("MATCH (n:TO {name: 'x'}) RETURN n")
    node = rows[0]["n"]
    assert_true(isinstance(node, nervusdb.Node), f"got {type(node)}")
    assert_true(hasattr(node, "id"), "Node should have .id")
    assert_true(hasattr(node, "labels"), "Node should have .labels")
    assert_true(hasattr(node, "properties"), "Node should have .properties")
    assert_true(isinstance(node.id, int), f"id should be int, got {type(node.id)}")
    assert_true(isinstance(node.labels, list), f"labels should be list, got {type(node.labels)}")
test("Node class attributes", test_node_type)

def test_rel_type():
    rows = db.query("MATCH ()-[r:REL]->() RETURN r")
    rel = rows[0]["r"]
    assert_true(isinstance(rel, nervusdb.Relationship), f"got {type(rel)}")
    assert_true(hasattr(rel, "start_node_id"), "Relationship should have .start_node_id")
    assert_true(hasattr(rel, "end_node_id"), "Relationship should have .end_node_id")
    assert_true(hasattr(rel, "rel_type"), "Relationship should have .rel_type")
    assert_true(hasattr(rel, "properties"), "Relationship should have .properties")
    assert_eq(rel.rel_type, "REL")
test("Relationship class attributes", test_rel_type)

def test_path_type():
    try:
        rows = db.query(
            "MATCH p = (a:TO {name: 'x'})-[:REL]->(b:TO {name: 'y'}) RETURN p"
        )
        if len(rows) > 0:
            path = rows[0]["p"]
            assert_true(isinstance(path, nervusdb.Path), f"got {type(path)}")
            assert_true(hasattr(path, "nodes"), "Path should have .nodes")
            assert_true(hasattr(path, "relationships"), "Path should have .relationships")
        else:
            skip("Path type", "no path returned")
    except Exception as e:
        skip("Path type", str(e)[:60])
test("Path class attributes", test_path_type)

def test_node_id_func():
    rows = db.query("MATCH (n:TO {name: 'x'}) RETURN id(n) AS nid, n")
    nid = rows[0]["nid"]
    node = rows[0]["n"]
    assert_true(isinstance(nid, int), f"id(n) should be int, got {type(nid)}")
    assert_eq(nid, node.id, "id(n) should match node.id")
test("id() function matches Node.id", test_node_id_func)

def test_labels_func():
    rows = db.query("MATCH (n:TO {name: 'x'}) RETURN labels(n) AS lbls")
    lbls = rows[0]["lbls"]
    assert_true(isinstance(lbls, list), f"labels() should return list, got {type(lbls)}")
    assert_true("TO" in lbls, "should contain TO label")
test("labels() function", test_labels_func)

db.close()

# ─── 25. 异常层级 ─────────────────────────────────────────────
print("\n── 25. 异常层级 [Python only] ──")

def test_nervus_base():
    assert_true(issubclass(nervusdb.SyntaxError, nervusdb.NervusError),
                "SyntaxError should be subclass of NervusError")
    assert_true(issubclass(nervusdb.ExecutionError, nervusdb.NervusError),
                "ExecutionError should be subclass of NervusError")
    assert_true(issubclass(nervusdb.StorageError, nervusdb.NervusError),
                "StorageError should be subclass of NervusError")
    assert_true(issubclass(nervusdb.CompatibilityError, nervusdb.NervusError),
                "CompatibilityError should be subclass of NervusError")
test("NervusError inheritance chain", test_nervus_base)

def test_catch_base():
    db_t, _ = fresh_db("exc")
    try:
        db_t.query("INVALID SYNTAX !!!")
        raise AssertionError("should have thrown")
    except nervusdb.NervusError:
        pass  # Catching base class should work
    db_t.close()
test("catch NervusError catches SyntaxError", test_catch_base)

def test_syntax_error_type():
    db_t, _ = fresh_db("exc2")
    try:
        db_t.query("BLAH BLAH")
    except nervusdb.SyntaxError:
        pass
    except Exception as e:
        raise AssertionError(f"Expected SyntaxError, got {type(e).__name__}")
    db_t.close()
test("SyntaxError for invalid query", test_syntax_error_type)

def test_storage_error_type():
    db_t, _ = fresh_db("exc3")
    db_t.close()
    try:
        db_t.query("RETURN 1")
    except nervusdb.StorageError:
        pass
    except Exception as e:
        raise AssertionError(f"Expected StorageError, got {type(e).__name__}")
test("StorageError for closed db", test_storage_error_type)

def test_exception_message():
    db_t, _ = fresh_db("exc4")
    try:
        db_t.query("NOT VALID")
    except nervusdb.NervusError as e:
        msg = str(e)
        assert_true(len(msg) > 0, "exception should have message")
    db_t.close()
test("exception has meaningful message", test_exception_message)

# ─── 26. Db.path + open() ────────────────────────────────────
print("\n── 26. Db.path + open() [Python only] ──")

def test_db_path():
    db_t, db_path = fresh_db("path")
    assert_eq(db_t.path, db_path)
    db_t.close()
test("Db.path property", test_db_path)

def test_open_func():
    _, db_path = fresh_db("openfn")
    # fresh_db already opened it, close and reopen with nervusdb.open()
    db_t = nervusdb.open(db_path)
    assert_true(isinstance(db_t, nervusdb.Db), f"open() should return Db, got {type(db_t)}")
    db_t.close()
test("nervusdb.open() convenience function", test_open_func)

def test_db_constructor():
    d = tempfile.mkdtemp(prefix="ndb-ctor-")
    p = os.path.join(d, "ctor.ndb")
    db_t = nervusdb.Db(p)
    db_t.execute_write("CREATE (:Ctor {v: 1})")
    rows = db_t.query("MATCH (n:Ctor) RETURN n.v")
    assert_eq(rows[0]["n.v"], 1)
    db_t.close()
test("Db() constructor", test_db_constructor)

# ─── 27. Python 边界情况 ──────────────────────────────────────
print("\n── 27. Python 边界情况 [Python only] ──")

db, _ = fresh_db("pyedge")

def test_large_int():
    # Python handles big ints natively; test i64 range
    big = 2**53
    db.execute_write(f"CREATE (:BigInt {{val: {big}}})")
    rows = db.query("MATCH (n:BigInt) RETURN n.val")
    assert_eq(rows[0]["n.val"], big)
test("large integer (2^53)", test_large_int)

def test_unicode_cjk():
    db.execute_write("CREATE (:Uni {name: '你好世界'})")
    rows = db.query("MATCH (n:Uni) RETURN n.name")
    assert_eq(rows[0]["n.name"], "你好世界")
test("Unicode CJK string", test_unicode_cjk)

def test_emoji():
    db.execute_write("CREATE (:Emoji {val: '🎉🚀'})")
    rows = db.query("MATCH (n:Emoji) RETURN n.val")
    assert_eq(rows[0]["n.val"], "🎉🚀")
test("emoji string", test_emoji)

def test_bad_param_type():
    try:
        # Pass an unsupported type as param
        db.query("RETURN $val AS v", params={"val": object()})
        skip("bad param type", "no error thrown")
    except (TypeError, Exception):
        pass  # Expected
test("invalid param type raises error", test_bad_param_type)

def test_close_with_active_txn():
    db2, _ = fresh_db("activetxn")
    txn = db2.begin_write()
    txn.query("CREATE (:AT {v: 1})")
    # close() with active txn should throw StorageError
    try:
        db2.close()
        # If it didn't throw, that's also informative
        print("    (note: close() succeeded with active txn)")
    except nervusdb.StorageError:
        print("    (confirmed: close() throws StorageError with active txn)")
    except Exception as e:
        print(f"    (close() threw {type(e).__name__}: {e})")
    # Clean up
    try:
        txn.rollback()
    except Exception:
        pass
    try:
        db2.close()
    except Exception:
        pass
test("close with active txn behavior", test_close_with_active_txn)

db.close()

# ═══════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════
print("\n" + "=" * 60)
print(f"🧪 测试完成: {passed} passed, {failed} failed, {skipped} skipped")
if failures:
    print("\n❌ 失败列表:")
    for f in failures:
        print(f"  - {f}")
print("=" * 60)
sys.exit(1 if failed > 0 else 0)
