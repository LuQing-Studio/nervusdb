#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "nervusdb.h"

// BEGIN: SHARED CID REGISTRY
// CID-SHARED-001 | mode=success | case=CREATE single node
// CID-SHARED-002 | mode=success | case=MATCH + RETURN node
// CID-SHARED-003 | mode=success | case=CREATE relationship
// CID-SHARED-004 | mode=success | case=SET property on node
// CID-SHARED-005 | mode=success | case=SET overwrite property
// CID-SHARED-006 | mode=success | case=REMOVE property
// CID-SHARED-007 | mode=success | case=DELETE node (detach)
// CID-SHARED-008 | mode=success | case=DELETE relationship only
// CID-SHARED-009 | mode=success | case=multi-node CREATE in single statement
// CID-SHARED-010 | mode=success | case=RETURN scalar expression
// CID-SHARED-011 | mode=success | case=RETURN property alias
// CID-SHARED-012 | mode=success | case=RETURN DISTINCT
// CID-SHARED-013 | mode=success | case=RETURN *
// CID-SHARED-014 | mode=success | case=CREATE node with multiple labels
// CID-SHARED-015 | mode=success | case=MATCH by single label subset
// CID-SHARED-016 | mode=success | case=null property
// CID-SHARED-017 | mode=success | case=boolean properties
// CID-SHARED-018 | mode=success | case=integer property
// CID-SHARED-019 | mode=success | case=negative integer
// CID-SHARED-020 | mode=success | case=float property
// CID-SHARED-021 | mode=success | case=string property with special chars
// CID-SHARED-022 | mode=success | case=list literal in RETURN
// CID-SHARED-023 | mode=success | case=map literal in RETURN
// CID-SHARED-024 | mode=success | case=list property on node
// CID-SHARED-025 | mode=success | case=WHERE equality
// CID-SHARED-026 | mode=success | case=WHERE comparison >
// CID-SHARED-027 | mode=success | case=WHERE AND
// CID-SHARED-028 | mode=success | case=WHERE OR
// CID-SHARED-029 | mode=success | case=WHERE NOT
// CID-SHARED-030 | mode=success | case=WHERE IN list
// CID-SHARED-031 | mode=success | case=WHERE STARTS WITH
// CID-SHARED-032 | mode=success | case=WHERE CONTAINS
// CID-SHARED-033 | mode=success | case=WHERE ENDS WITH
// CID-SHARED-034 | mode=success | case=WHERE IS NULL
// CID-SHARED-035 | mode=success | case=WHERE IS NOT NULL
// CID-SHARED-036 | mode=success | case=ORDER BY ASC
// CID-SHARED-037 | mode=success | case=ORDER BY DESC
// CID-SHARED-038 | mode=success | case=LIMIT
// CID-SHARED-039 | mode=success | case=SKIP
// CID-SHARED-040 | mode=success | case=WITH pipe
// CID-SHARED-041 | mode=success | case=UNWIND
// CID-SHARED-042 | mode=success | case=UNWIND + CREATE
// CID-SHARED-043 | mode=success | case=UNION
// CID-SHARED-044 | mode=success | case=UNION ALL
// CID-SHARED-045 | mode=success | case=OPTIONAL MATCH
// CID-SHARED-046 | mode=success | case=count()
// CID-SHARED-047 | mode=success | case=sum()
// CID-SHARED-048 | mode=success | case=avg()
// CID-SHARED-049 | mode=success | case=min() / max()
// CID-SHARED-050 | mode=success | case=collect()
// CID-SHARED-051 | mode=success | case=count(DISTINCT)
// CID-SHARED-052 | mode=success | case=GROUP BY (implicit)
// CID-SHARED-053 | mode=success | case=MERGE creates when not exists
// CID-SHARED-054 | mode=success | case=MERGE matches when exists
// CID-SHARED-055 | mode=success | case=MERGE ON CREATE SET
// CID-SHARED-056 | mode=success | case=MERGE ON MATCH SET
// CID-SHARED-057 | mode=success | case=MERGE relationship
// CID-SHARED-058 | mode=success | case=simple CASE
// CID-SHARED-059 | mode=success | case=generic CASE
// CID-SHARED-060 | mode=success | case=toString()
// CID-SHARED-061 | mode=success | case=toUpper / toLower
// CID-SHARED-062 | mode=success | case=trim / lTrim / rTrim
// CID-SHARED-063 | mode=success | case=substring
// CID-SHARED-064 | mode=success | case=size() on string
// CID-SHARED-065 | mode=success | case=replace()
// CID-SHARED-066 | mode=success | case=left / right
// CID-SHARED-067 | mode=success | case=arithmetic: + - * / %
// CID-SHARED-068 | mode=success | case=abs()
// CID-SHARED-069 | mode=success | case=toInteger / toFloat
// CID-SHARED-070 | mode=success | case=sign()
// CID-SHARED-071 | mode=success | case=fixed length path *2
// CID-SHARED-072 | mode=success | case=variable length path *1..3
// CID-SHARED-073 | mode=success | case=variable length path *..2
// CID-SHARED-074 | mode=success | case=shortest path
// CID-SHARED-075 | mode=success | case=WHERE EXISTS pattern
// CID-SHARED-076 | mode=success | case=FOREACH create nodes
// CID-SHARED-077 | mode=success | case=beginWrite + query + commit
// CID-SHARED-078 | mode=success | case=rollback discards staged queries
// CID-SHARED-079 | mode=error | case=txn syntax error at query time
// CID-SHARED-080 | mode=success | case=multiple txn commits are independent
// CID-SHARED-081 | mode=error | case=syntax error in query() -> SyntaxError
// CID-SHARED-082 | mode=error | case=syntax error in execute_write() -> SyntaxError
// CID-SHARED-083 | mode=error | case=write via query() is rejected
// CID-SHARED-084 | mode=error | case=error is typed NervusError subclass
// CID-SHARED-085 | mode=error | case=operations after close() throw StorageError
// CID-SHARED-086 | mode=error | case=double close is safe
// CID-SHARED-087 | mode=success | case=outgoing match ->
// CID-SHARED-088 | mode=success | case=incoming match <-
// CID-SHARED-089 | mode=success | case=undirected match -[]-
// CID-SHARED-090 | mode=success | case=relationship properties
// CID-SHARED-091 | mode=success | case=triangle pattern
// CID-SHARED-092 | mode=success | case=multi-hop with WHERE
// CID-SHARED-093 | mode=success | case=multiple MATCH clauses
// CID-SHARED-094 | mode=success | case=batch create 1000 nodes
// CID-SHARED-095 | mode=success | case=batch query 1000 nodes
// CID-SHARED-096 | mode=success | case=UNWIND batch create
// CID-SHARED-097 | mode=success | case=data survives close + reopen
// CID-SHARED-098 | mode=success | case=empty result set
// CID-SHARED-099 | mode=success | case=RETURN literal without MATCH
// CID-SHARED-100 | mode=success | case=empty string property
// CID-SHARED-101 | mode=success | case=large string property
// CID-SHARED-102 | mode=success | case=node with many properties
// CID-SHARED-103 | mode=success | case=self-loop relationship
// CID-SHARED-104 | mode=success | case=UNWIND ordered
// CID-SHARED-105 | mode=success | case=UNWIND empty list
// CID-SHARED-106 | mode=success | case=UNWIND with aggregation
// CID-SHARED-107 | mode=success | case=UNWIND + CREATE
// CID-SHARED-108 | mode=success | case=UNWIND range()
// CID-SHARED-109 | mode=success | case=UNION dedup
// CID-SHARED-110 | mode=success | case=UNION ALL keeps dupes
// CID-SHARED-111 | mode=success | case=multi UNION
// CID-SHARED-112 | mode=success | case=UNION with MATCH
// CID-SHARED-113 | mode=success | case=WITH multi-stage pipeline
// CID-SHARED-114 | mode=success | case=WITH DISTINCT
// CID-SHARED-115 | mode=success | case=WITH + aggregation
// CID-SHARED-116 | mode=success | case=pagination page 1
// CID-SHARED-117 | mode=success | case=pagination page 2
// CID-SHARED-118 | mode=success | case=SKIP beyond results
// CID-SHARED-119 | mode=success | case=ORDER BY multi-column
// CID-SHARED-120 | mode=success | case=COALESCE
// CID-SHARED-121 | mode=success | case=COALESCE first non-null
// CID-SHARED-122 | mode=success | case=null + 1 propagation
// CID-SHARED-123 | mode=success | case=null = null
// CID-SHARED-124 | mode=success | case=IS NULL filter
// CID-SHARED-125 | mode=success | case=IS NOT NULL filter
// CID-SHARED-126 | mode=success | case=toInteger(3.9)
// CID-SHARED-127 | mode=success | case=toInteger('42')
// CID-SHARED-128 | mode=success | case=toFloat(42)
// CID-SHARED-129 | mode=success | case=toFloat('3.14')
// CID-SHARED-130 | mode=success | case=toString(42)
// CID-SHARED-131 | mode=success | case=toString(true)
// CID-SHARED-132 | mode=success | case=toBoolean('true')
// CID-SHARED-133 | mode=success | case=abs(-7)
// CID-SHARED-134 | mode=success | case=ceil(2.3)
// CID-SHARED-135 | mode=success | case=floor(2.7)
// CID-SHARED-136 | mode=success | case=round(2.5)
// CID-SHARED-137 | mode=success | case=sign()
// CID-SHARED-138 | mode=success | case=sqrt(16)
// CID-SHARED-139 | mode=success | case=log(1)
// CID-SHARED-140 | mode=success | case=e()
// CID-SHARED-141 | mode=success | case=pi()
// CID-SHARED-142 | mode=success | case=replace()
// CID-SHARED-143 | mode=success | case=lTrim()
// CID-SHARED-144 | mode=success | case=rTrim()
// CID-SHARED-145 | mode=success | case=split()
// CID-SHARED-146 | mode=success | case=reverse()
// CID-SHARED-147 | mode=success | case=substring()
// CID-SHARED-148 | mode=success | case=range(1, 5)
// CID-SHARED-149 | mode=success | case=range(0, 10, 2)
// CID-SHARED-150 | mode=success | case=list index access
// CID-SHARED-151 | mode=success | case=size() on list
// CID-SHARED-152 | mode=success | case=list comprehension
// CID-SHARED-153 | mode=success | case=reduce()
// CID-SHARED-154 | mode=success | case=map literal
// CID-SHARED-155 | mode=success | case=map property access
// CID-SHARED-156 | mode=success | case=nested map
// CID-SHARED-157 | mode=success | case=keys() on map
// CID-SHARED-158 | mode=success | case=cartesian product
// CID-SHARED-159 | mode=success | case=correlated MATCH
// CID-SHARED-160 | mode=success | case=independent MATCH
// CID-SHARED-161 | mode=success | case=REMOVE property
// CID-SHARED-162 | mode=success | case=REMOVE multiple properties
// CID-SHARED-163 | mode=success | case=REMOVE label
// CID-SHARED-164 | mode=success | case=$param in WHERE
// CID-SHARED-165 | mode=success | case=$param in CREATE
// CID-SHARED-166 | mode=success | case=multiple $params
// CID-SHARED-167 | mode=success | case=$param string
// CID-SHARED-168 | mode=success | case=EXPLAIN basic
// CID-SHARED-169 | mode=success | case=index-accelerated lookup
// CID-SHARED-170 | mode=success | case=index with post-creation inserts
// CID-SHARED-171 | mode=success | case=index range query
// CID-SHARED-172 | mode=success | case=snapshot isolation across writes
// CID-SHARED-173 | mode=success | case=snapshot isolation across handles
// CID-SHARED-174 | mode=error | case=type error in arithmetic
// CID-SHARED-175 | mode=error | case=division by zero
// CID-SHARED-176 | mode=error | case=missing property returns null
// CID-SHARED-177 | mode=error | case=syntax error detail
// CID-SHARED-178 | mode=error | case=unknown function error
// CID-SHARED-179 | mode=error | case=delete connected node error
// END: SHARED CID REGISTRY


static void expect(int cond, const char* msg) {
  if (!cond) {
    char err_buf[1024];
    ndb_last_error_message(err_buf, sizeof(err_buf));
    fprintf(stderr, "[c-smoke] FAIL: %s\n", msg);
    fprintf(stderr, "[c-smoke] last_error code=%d category=%d message=%s\n",
            ndb_last_error_code(),
            ndb_last_error_category(),
            err_buf);
    exit(1);
  }
}

int main(void) {
  char db_path[512];
  snprintf(db_path, sizeof(db_path), "/tmp/nervusdb-c-smoke-%d.ndb", (int)getpid());

  ndb_db_t* db = NULL;
  expect(ndb_open(db_path, &db) == NDB_OK, "ndb_open should succeed");
  expect(db != NULL, "db handle should not be null");

  uint32_t affected = 0;
  expect(ndb_execute_write(db,
                           "CREATE (:User {name: 'alice'})",
                           NULL,
                           &affected) == NDB_OK,
         "execute_write CREATE should succeed");
  expect(affected >= 1, "affected rows should be >= 1");

  ndb_result_t* result = NULL;
  expect(ndb_query(db,
                   "MATCH (n:User) RETURN count(n) AS c",
                   NULL,
                   &result) == NDB_OK,
         "query should succeed");
  expect(result != NULL, "query result should not be null");

  char* json = NULL;
  expect(ndb_result_to_json(result, &json) == NDB_OK, "result_to_json should succeed");
  expect(json != NULL, "json should not be null");
  expect(strstr(json, "\"c\":1") != NULL || strstr(json, "\"c\":1.0") != NULL,
         "count should be 1");
  ndb_string_free(json);
  ndb_result_free(result);

  ndb_txn_t* txn = NULL;
  expect(ndb_begin_write(db, &txn) == NDB_OK, "begin_write should succeed");
  expect(txn != NULL, "txn handle should not be null");
  expect(ndb_txn_query(txn, "CREATE (:User {name: 'bob'})", NULL) == NDB_OK,
         "txn query should succeed");
  expect(ndb_txn_commit(txn) == NDB_OK, "txn commit should succeed");

  result = NULL;
  expect(ndb_query(db,
                   "MATCH (n:User) RETURN count(n) AS c",
                   NULL,
                   &result) == NDB_OK,
         "query after txn should succeed");
  json = NULL;
  expect(ndb_result_to_json(result, &json) == NDB_OK, "result_to_json after txn should succeed");
  expect(strstr(json, "\"c\":2") != NULL || strstr(json, "\"c\":2.0") != NULL,
         "count should be 2 after txn commit");
  ndb_string_free(json);
  ndb_result_free(result);

  expect(ndb_close(db) == NDB_OK, "close should succeed");

  printf("c-binding-smoke ok\n");
  return 0;
}
