use super::Value;
use super::evaluator_equality::cypher_equals;

pub(super) fn string_predicate<F>(left: &Value, right: &Value, pred: F) -> Value
where
    F: FnOnce(&str, &str) -> bool,
{
    match (left, right) {
        (Value::String(l), Value::String(r)) => Value::Bool(pred(l, r)),
        (Value::Null, _) | (_, Value::Null) => Value::Null,
        _ => Value::Null,
    }
}

pub(super) fn in_list(left: &Value, right: &Value) -> Value {
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => Value::Null,
        (l, Value::List(items)) => {
            let mut saw_null = false;
            for item in items {
                match cypher_equals(l, item) {
                    Value::Bool(true) => return Value::Bool(true),
                    Value::Bool(false) => {}
                    Value::Null => saw_null = true,
                    _ => saw_null = true,
                }
            }
            if saw_null {
                Value::Null
            } else {
                Value::Bool(false)
            }
        }
        _ => Value::Null,
    }
}
