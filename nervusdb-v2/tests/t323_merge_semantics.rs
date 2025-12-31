use nervusdb_v2::Db;
use nervusdb_v2_query::{Params, Result, Value, prepare};
use tempfile::tempdir;

#[test]
fn t323_merge_on_create_on_match_node() {
    let dir = tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let q = prepare(
        "MERGE (n {name: 'Alice'}) \
         ON CREATE SET n.age = 1 \
         ON MATCH SET n.age = 2",
    )
    .unwrap();

    {
        let mut txn = db.begin_write();
        let created = q
            .execute_write(&db.snapshot(), &mut txn, &Params::new())
            .unwrap();
        txn.commit().unwrap();
        assert_eq!(created, 1);
    }

    {
        let snapshot = db.snapshot();
        let q2 = prepare("MATCH (n) WHERE n.name = 'Alice' RETURN n.age AS age").unwrap();
        let rows: Vec<_> = q2
            .execute_streaming(&snapshot, &Params::new())
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("age"), Some(&Value::Int(1)));
    }

    {
        let mut txn = db.begin_write();
        let created = q
            .execute_write(&db.snapshot(), &mut txn, &Params::new())
            .unwrap();
        txn.commit().unwrap();
        assert_eq!(created, 0);
    }

    {
        let snapshot = db.snapshot();
        let q2 = prepare("MATCH (n) WHERE n.name = 'Alice' RETURN n.age AS age").unwrap();
        let rows: Vec<_> = q2
            .execute_streaming(&snapshot, &Params::new())
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("age"), Some(&Value::Int(2)));
    }
}

#[test]
fn t323_merge_on_create_on_match_edge() {
    let dir = tempdir().unwrap();
    let db = Db::open(dir.path()).unwrap();

    let q = prepare(
        "MERGE (a {name: 'A'})-[r:1]->(b {name: 'B'}) \
         ON CREATE SET r.weight = 1 \
         ON MATCH SET r.weight = 2",
    )
    .unwrap();

    {
        let mut txn = db.begin_write();
        let created = q
            .execute_write(&db.snapshot(), &mut txn, &Params::new())
            .unwrap();
        txn.commit().unwrap();
        assert_eq!(created, 3);
    }

    {
        let snapshot = db.snapshot();
        let q2 = prepare("MATCH (a)-[r:1]->(b) RETURN r.weight AS w").unwrap();
        let rows: Vec<_> = q2
            .execute_streaming(&snapshot, &Params::new())
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("w"), Some(&Value::Int(1)));
    }

    {
        let mut txn = db.begin_write();
        let created = q
            .execute_write(&db.snapshot(), &mut txn, &Params::new())
            .unwrap();
        txn.commit().unwrap();
        assert_eq!(created, 0);
    }

    {
        let snapshot = db.snapshot();
        let q2 = prepare("MATCH (a)-[r:1]->(b) RETURN r.weight AS w").unwrap();
        let rows: Vec<_> = q2
            .execute_streaming(&snapshot, &Params::new())
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("w"), Some(&Value::Int(2)));
    }
}
