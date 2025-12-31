use nervusdb_v2::Db;
use nervusdb_v2::query::Value;
use tempfile::tempdir;

#[test]
#[ignore] // Blocked on parser support for EXISTS { match }
fn test_exists_with_edge() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t309.ndb");
    let db = Db::open(&db_path)?;
    let mut txn = db.begin_write();

    let label = txn.get_or_create_label("Person")?;
    let rel_type = txn.get_or_create_rel_type("KNOWS")?;
    let n1 = txn.create_node(label.into(), 1)?;
    txn.set_node_property(
        n1,
        "name".to_string(),
        nervusdb_v2::PropertyValue::String("Alice".to_string()),
    )?;
    let n2 = txn.create_node(label.into(), 2)?;
    txn.set_node_property(
        n2,
        "name".to_string(),
        nervusdb_v2::PropertyValue::String("Bob".to_string()),
    )?;
    // Alice knows Bob
    txn.create_edge(n1, n2, rel_type.into());
    txn.commit()?;

    // MATCH (n:Person) WHERE EXISTS ((n)-[:KNOWS]->()) RETURN n.name
    let query = "MATCH (n:Person) WHERE EXISTS { (n)-[:KNOWS]->() } RETURN n.name AS name";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    // Only Alice has outgoing KNOWS edge
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].get("name").unwrap(),
        &Value::String("Alice".to_string())
    );

    Ok(())
}

#[test]
#[ignore] // Blocked on parser support for EXISTS { match }
fn test_exists_no_edge() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t309.ndb");
    let db = Db::open(&db_path)?;
    let mut txn = db.begin_write();

    let label = txn.get_or_create_label("Person")?;
    let n1 = txn.create_node(label.into(), 1)?;
    txn.set_node_property(
        n1,
        "name".to_string(),
        nervusdb_v2::PropertyValue::String("Lonely".to_string()),
    )?;
    txn.commit()?;

    // No edges, EXISTS should filter out everyone
    let query = "MATCH (n:Person) WHERE EXISTS { (n)-[:KNOWS]->() } RETURN n.name AS name";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 0);

    Ok(())
}

#[test]
#[ignore] // Blocked on parser support for EXISTS { match }
fn test_not_exists() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t309.ndb");
    let db = Db::open(&db_path)?;
    let mut txn = db.begin_write();

    let label = txn.get_or_create_label("Person")?;
    let rel_type = txn.get_or_create_rel_type("KNOWS")?;
    let n1 = txn.create_node(label.into(), 1)?;
    txn.set_node_property(
        n1,
        "name".to_string(),
        nervusdb_v2::PropertyValue::String("Alice".to_string()),
    )?;
    let n2 = txn.create_node(label.into(), 2)?;
    txn.set_node_property(
        n2,
        "name".to_string(),
        nervusdb_v2::PropertyValue::String("Bob".to_string()),
    )?;
    txn.create_edge(n1, n2, rel_type.into());
    txn.commit()?;

    // NOT EXISTS - Bob doesn't have outgoing KNOWS
    let query = "MATCH (n:Person) WHERE NOT EXISTS { (n)-[:KNOWS]->() } RETURN n.name AS name";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    // Only Bob has no outgoing KNOWS edge
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].get("name").unwrap(),
        &Value::String("Bob".to_string())
    );

    Ok(())
}
