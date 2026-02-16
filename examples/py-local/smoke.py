import nervusdb


def main() -> None:
    db = nervusdb.open("/tmp/nervusdb-py-local.ndb")
    db.execute_write("CREATE (n:Person {name:'Py Local'})")
    rows = list(db.query_stream("MATCH (n:Person) RETURN n LIMIT 1"))
    if not rows:
        raise RuntimeError("expected non-empty result rows")
    db.close()
    print("py-local smoke ok")


if __name__ == "__main__":
    main()
