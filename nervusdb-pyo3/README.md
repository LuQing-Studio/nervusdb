# nervusdb

An embeddable property graph database written in Rust with Python bindings.

## Installation

```bash
pip install nervusdb
```

## Usage

```python
import nervusdb

# Open or create a database
db = nervusdb.open("my_graph.ndb")

# Create nodes using a write transaction
txn = db.begin_write()
txn.query("CREATE (n:Person {name: 'Alice', age: 30})")
txn.commit()

# Query the graph
result = db.query("MATCH (n:Person) RETURN n.name, n.age")
for row in result:
    print(row)

# Close the database
db.close()
```

## License

MIT
