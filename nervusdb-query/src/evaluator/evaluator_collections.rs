use super::Value;
use crate::executor::convert_api_property_to_value;
use nervusdb_api::GraphSnapshot;

pub(super) fn evaluate_collection_function<S: GraphSnapshot>(
    name: &str,
    args: &[Value],
    snapshot: &S,
) -> Option<Value> {
    match name {
        "size" => Some(evaluate_size(args)),
        "head" => Some(evaluate_head(args)),
        "tail" => Some(evaluate_tail(args)),
        "last" => Some(evaluate_last(args)),
        "keys" => Some(evaluate_keys(args, snapshot)),
        "length" => Some(evaluate_length(args)),
        "nodes" => Some(evaluate_nodes(args)),
        "relationships" => Some(evaluate_relationships(args)),
        "range" => Some(evaluate_range(args)),
        "__index" => Some(evaluate_index(args, snapshot)),
        "__slice" => Some(evaluate_slice(args)),
        "__getprop" => Some(evaluate_getprop(args, snapshot)),
        "properties" => Some(evaluate_properties(args, snapshot)),
        _ => None,
    }
}

fn evaluate_size(args: &[Value]) -> Value {
    match args.first() {
        Some(Value::List(items)) => Value::Int(items.len() as i64),
        Some(Value::String(text)) => Value::Int(text.chars().count() as i64),
        Some(Value::Map(map)) => Value::Int(map.len() as i64),
        _ => Value::Null,
    }
}

fn evaluate_head(args: &[Value]) -> Value {
    if let Some(Value::List(items)) = args.first() {
        items.first().cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn evaluate_tail(args: &[Value]) -> Value {
    if let Some(Value::List(items)) = args.first() {
        if items.len() > 1 {
            Value::List(items[1..].to_vec())
        } else {
            Value::List(vec![])
        }
    } else {
        Value::Null
    }
}

fn evaluate_last(args: &[Value]) -> Value {
    if let Some(Value::List(items)) = args.first() {
        items.last().cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn evaluate_keys<S: GraphSnapshot>(args: &[Value], snapshot: &S) -> Value {
    match args.first() {
        Some(Value::Map(map)) => {
            let keys: Vec<Value> = map.keys().map(|key| Value::String(key.clone())).collect();
            Value::List(keys)
        }
        Some(Value::Node(node)) => {
            let keys: Vec<Value> = node
                .properties
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect();
            Value::List(keys)
        }
        Some(Value::Relationship(rel)) => {
            let keys: Vec<Value> = rel
                .properties
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect();
            Value::List(keys)
        }
        Some(Value::NodeId(id)) => {
            if let Some(props) = snapshot.node_properties(*id) {
                let keys: Vec<Value> = props.keys().map(|key| Value::String(key.clone())).collect();
                Value::List(keys)
            } else {
                Value::List(vec![])
            }
        }
        Some(Value::EdgeKey(key)) => {
            if let Some(props) = snapshot.edge_properties(*key) {
                let keys: Vec<Value> = props.keys().map(|key| Value::String(key.clone())).collect();
                Value::List(keys)
            } else {
                Value::List(vec![])
            }
        }
        _ => Value::Null,
    }
}

fn evaluate_length(args: &[Value]) -> Value {
    if let Some(Value::Path(path)) = args.first() {
        Value::Int(path.edges.len() as i64)
    } else {
        Value::Null
    }
}

fn evaluate_nodes(args: &[Value]) -> Value {
    if let Some(Value::Path(path)) = args.first() {
        Value::List(path.nodes.iter().map(|id| Value::NodeId(*id)).collect())
    } else {
        Value::Null
    }
}

fn evaluate_relationships(args: &[Value]) -> Value {
    if let Some(Value::Path(path)) = args.first() {
        Value::List(
            path.edges
                .iter()
                .map(|edge| Value::EdgeKey(*edge))
                .collect(),
        )
    } else {
        Value::Null
    }
}

fn evaluate_range(args: &[Value]) -> Value {
    if args.len() < 2 || args.len() > 3 {
        return Value::Null;
    }

    let start = match args[0] {
        Value::Int(v) => v,
        _ => return Value::Null,
    };
    let end = match args[1] {
        Value::Int(v) => v,
        _ => return Value::Null,
    };

    let step = if args.len() == 3 {
        match args[2] {
            Value::Int(v) => v,
            _ => return Value::Null,
        }
    } else if start <= end {
        1
    } else {
        -1
    };

    if step == 0 {
        return Value::Null;
    }

    let mut out = Vec::new();
    let mut current = start;
    if step > 0 {
        while current <= end {
            out.push(Value::Int(current));
            current = match current.checked_add(step) {
                Some(next) => next,
                None => break,
            };
        }
    } else {
        while current >= end {
            out.push(Value::Int(current));
            current = match current.checked_add(step) {
                Some(next) => next,
                None => break,
            };
        }
    }

    Value::List(out)
}

fn evaluate_index<S: GraphSnapshot>(args: &[Value], snapshot: &S) -> Value {
    if args.len() != 2 {
        return Value::Null;
    }

    match (&args[0], &args[1]) {
        (Value::List(items), Value::Int(index)) => {
            let len = items.len() as i64;
            let idx = if *index < 0 { len + *index } else { *index };
            if idx < 0 || idx >= len {
                Value::Null
            } else {
                items[idx as usize].clone()
            }
        }
        (Value::String(text), Value::Int(index)) => {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len() as i64;
            let idx = if *index < 0 { len + *index } else { *index };
            if idx < 0 || idx >= len {
                Value::Null
            } else {
                Value::String(chars[idx as usize].to_string())
            }
        }
        (Value::Map(map), Value::String(key)) => map.get(key).cloned().unwrap_or(Value::Null),
        (Value::Node(node), Value::String(key)) => {
            node.properties.get(key).cloned().unwrap_or(Value::Null)
        }
        (Value::Relationship(rel), Value::String(key)) => {
            rel.properties.get(key).cloned().unwrap_or(Value::Null)
        }
        (Value::NodeId(id), Value::String(key)) => snapshot
            .node_property(*id, key)
            .as_ref()
            .map(convert_api_property_to_value)
            .unwrap_or(Value::Null),
        (Value::EdgeKey(edge), Value::String(key)) => snapshot
            .edge_property(*edge, key)
            .as_ref()
            .map(convert_api_property_to_value)
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn evaluate_slice(args: &[Value]) -> Value {
    if args.len() != 3 {
        return Value::Null;
    }

    let parse_index = |value: &Value| -> Option<i64> {
        match value {
            Value::Null => None,
            Value::Int(v) => Some(*v),
            _ => None,
        }
    };

    let start = parse_index(&args[1]);
    let end = parse_index(&args[2]);

    match &args[0] {
        Value::List(items) => {
            let len = items.len() as i64;
            let normalize = |idx: Option<i64>, default: i64| -> i64 {
                match idx {
                    Some(i) if i < 0 => (len + i).clamp(0, len),
                    Some(i) => i.clamp(0, len),
                    None => default,
                }
            };
            let from = normalize(start, 0);
            let to = normalize(end, len);
            if to < from {
                Value::List(vec![])
            } else {
                Value::List(items[from as usize..to as usize].to_vec())
            }
        }
        Value::String(text) => {
            let chars: Vec<char> = text.chars().collect();
            let len = chars.len() as i64;
            let normalize = |idx: Option<i64>, default: i64| -> i64 {
                match idx {
                    Some(i) if i < 0 => (len + i).clamp(0, len),
                    Some(i) => i.clamp(0, len),
                    None => default,
                }
            };
            let from = normalize(start, 0);
            let to = normalize(end, len);
            if to < from {
                Value::String(String::new())
            } else {
                Value::String(chars[from as usize..to as usize].iter().collect())
            }
        }
        _ => Value::Null,
    }
}

fn evaluate_getprop<S: GraphSnapshot>(args: &[Value], snapshot: &S) -> Value {
    if args.len() != 2 {
        return Value::Null;
    }

    let key = match &args[1] {
        Value::String(s) => s,
        _ => return Value::Null,
    };

    match &args[0] {
        Value::Map(map) => map.get(key).cloned().unwrap_or(Value::Null),
        Value::Node(node) => node.properties.get(key).cloned().unwrap_or(Value::Null),
        Value::Relationship(rel) => rel.properties.get(key).cloned().unwrap_or(Value::Null),
        Value::NodeId(id) => snapshot
            .node_property(*id, key)
            .as_ref()
            .map(convert_api_property_to_value)
            .unwrap_or(Value::Null),
        Value::EdgeKey(edge) => snapshot
            .edge_property(*edge, key)
            .as_ref()
            .map(convert_api_property_to_value)
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn evaluate_properties<S: GraphSnapshot>(args: &[Value], snapshot: &S) -> Value {
    match args.first() {
        Some(Value::Map(map)) => Value::Map(map.clone()),
        Some(Value::Node(node)) => Value::Map(node.properties.clone()),
        Some(Value::Relationship(rel)) => Value::Map(rel.properties.clone()),
        Some(Value::NodeId(id)) => {
            if let Some(props) = snapshot.node_properties(*id) {
                let mut out = std::collections::BTreeMap::new();
                for (key, value) in props {
                    out.insert(key, convert_api_property_to_value(&value));
                }
                Value::Map(out)
            } else {
                Value::Null
            }
        }
        Some(Value::EdgeKey(key)) => {
            if let Some(props) = snapshot.edge_properties(*key) {
                let mut out = std::collections::BTreeMap::new();
                for (prop_key, prop_value) in props {
                    out.insert(prop_key, convert_api_property_to_value(&prop_value));
                }
                Value::Map(out)
            } else {
                Value::Null
            }
        }
        Some(Value::Null) => Value::Null,
        _ => Value::Null,
    }
}
