use nervusdb_v2::{Db, GraphSnapshot, PropertyValue};

#[test]
fn test_set_clause_index_update() -> nervusdb_v2::Result<()> {
    let dir = tempfile::tempdir()?;
    let db_path = dir.path().join("t108.ndb");
    let db = Db::open(&db_path)?;

    // 1. Create Index
    db.create_index("Person", "name")?;

    // 2. Insert Initial Data
    {
        let mut txn = db.begin_write();
        let label = txn.get_or_create_label("Person")?;
        let iid = txn.create_node(1, label)?;
        txn.set_node_property(
            iid,
            "name".to_string(),
            PropertyValue::String("Alice".to_string()),
        )?;
        txn.commit()?;
    }

    let snapshot = db.snapshot();
    // Verify initial index lookup
    let ids = snapshot
        .lookup_index(
            "Person",
            "name",
            &PropertyValue::String("Alice".to_string()),
        )
        .expect("Index should exist");
    assert_eq!(ids.len(), 1);
    let alice_node_id = ids[0];

    // 3. Update Property via SET clause
    {
        // Use a snapshot for reading (outside write txn to avoid potential contention if any,
        // though MVCC should allow it).
        let write_snapshot = db.snapshot();

        let mut txn = db.begin_write();
        let query = "MATCH (n:Person) WHERE n.name = 'Alice' SET n.name = 'Bob'";
        let prepared = nervusdb_v2::query::prepare(query)?;

        // Executing write query (1 property set)
        let count = prepared.execute_write(&write_snapshot, &mut txn, &Default::default())?;
        assert_eq!(count, 1);

        txn.commit()?;
    }

    // 4. Verify Index Update (New Snapshot)
    let snapshot = db.snapshot();

    // "Alice" should be gone from index
    let alice_lookup = snapshot.lookup_index(
        "Person",
        "name",
        &PropertyValue::String("Alice".to_string()),
    );
    if let Some(ids) = alice_lookup {
        assert!(
            ids.is_empty(),
            "Alice should be removed from index, found: {:?}",
            ids
        );
    }

    // "Bob" should be in index
    let bob_lookup = snapshot
        .lookup_index("Person", "name", &PropertyValue::String("Bob".to_string()))
        .expect("Index should still exist");
    assert_eq!(bob_lookup.len(), 1, "Bob should be in index");
    assert_eq!(bob_lookup[0], alice_node_id, "Node ID should be preserved");

    // 5. Verify Property Value
    let val = snapshot
        .node_property(alice_node_id, "name")
        .expect("Property should exist");
    match val {
        PropertyValue::String(s) => assert_eq!(s, "Bob"),
        _ => panic!("Wrong type"),
    }

    Ok(())
}
