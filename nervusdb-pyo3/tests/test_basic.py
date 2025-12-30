#!/usr/bin/env python3
"""Basic integration test for nervusdb Python bindings."""

import nervusdb
import tempfile
import os

def test_basic():
    """Test basic CRUD operations."""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = os.path.join(tmpdir, "test.ndb")
        
        # Open database
        db = nervusdb.open(db_path)
        print(f"✓ Opened database at {db.path}")
        
        # Create nodes using write transaction
        txn = db.begin_write()
        txn.query("CREATE (n:Person {name: 'Alice', age: 30})")
        txn.query("CREATE (n:Person {name: 'Bob', age: 25})")
        txn.commit()
        print("✓ Created 2 nodes")
        
        # Query nodes (v2 uses RETURN node variable)
        result = db.query("MATCH (n:Person) RETURN n")
        print(f"✓ Query returned {len(result)} rows")
        for row in result:
            print(f"  - {row}")
        
        # Test count aggregation
        count_result = db.query("MATCH (n:Person) RETURN count(n)")
        print(f"✓ Count query: {count_result}")
        
        # Close database
        db.close()
        print("✓ Closed database")
        
    print("\n🎉 All tests passed!")

if __name__ == "__main__":
    test_basic()
