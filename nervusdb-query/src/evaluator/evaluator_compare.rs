use super::evaluator_numeric::value_as_f64;
use super::evaluator_temporal_math::{compare_time_of_day, compare_time_with_offset};
use super::evaluator_temporal_parse::parse_temporal_string;
use super::{TemporalValue, Value};
use std::cmp::Ordering;

pub(super) fn compare_values<F>(left: &Value, right: &Value, cmp: F) -> Value
where
    F: Fn(Ordering) -> bool,
{
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => Value::Null,
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
            compare_numbers_for_range(left, right, &cmp)
        }
        (Value::Bool(l), Value::Bool(r)) => Value::Bool(cmp(l.cmp(r))),
        (Value::String(l), Value::String(r)) => {
            Value::Bool(cmp(compare_strings_with_temporal(l, r)))
        }
        (Value::List(l), Value::List(r)) => compare_lists_for_range(l, r, &cmp),
        _ => Value::Null,
    }
}

fn compare_numbers_for_range<F>(left: &Value, right: &Value, cmp: &F) -> Value
where
    F: Fn(Ordering) -> bool,
{
    let (l, r) = match (value_as_f64(left), value_as_f64(right)) {
        (Some(l), Some(r)) => (l, r),
        _ => return Value::Null,
    };
    if l.is_nan() || r.is_nan() {
        return Value::Bool(false);
    }
    l.partial_cmp(&r)
        .map(|ord| Value::Bool(cmp(ord)))
        .unwrap_or(Value::Null)
}

fn compare_lists_for_range<F>(left: &[Value], right: &[Value], cmp: &F) -> Value
where
    F: Fn(Ordering) -> bool,
{
    for (l, r) in left.iter().zip(right.iter()) {
        match compare_value_for_list_ordering(l, r) {
            Some(Ordering::Equal) => {}
            Some(ord) => return Value::Bool(cmp(ord)),
            None => return Value::Null,
        }
    }
    Value::Bool(cmp(left.len().cmp(&right.len())))
}

fn compare_value_for_list_ordering(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Null, _) | (_, Value::Null) => None,
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => {
            let l = value_as_f64(left)?;
            let r = value_as_f64(right)?;
            if l.is_nan() || r.is_nan() {
                None
            } else {
                l.partial_cmp(&r)
            }
        }
        (Value::Bool(l), Value::Bool(r)) => Some(l.cmp(r)),
        (Value::String(l), Value::String(r)) => Some(compare_strings_with_temporal(l, r)),
        (Value::List(l), Value::List(r)) => compare_lists_ordering(l, r),
        _ => None,
    }
}

fn compare_lists_ordering(left: &[Value], right: &[Value]) -> Option<Ordering> {
    for (l, r) in left.iter().zip(right.iter()) {
        match compare_value_for_list_ordering(l, r) {
            Some(Ordering::Equal) => {}
            non_eq => return non_eq,
        }
    }
    Some(left.len().cmp(&right.len()))
}

pub(super) fn order_compare_non_null(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Bool(l), Value::Bool(r)) => Some(l.cmp(r)),
        (Value::Int(l), Value::Int(r)) => Some(l.cmp(r)),
        (Value::Float(l), Value::Float(r)) => l.partial_cmp(r),
        (Value::Int(l), Value::Float(r)) => (*l as f64).partial_cmp(r),
        (Value::Float(l), Value::Int(r)) => l.partial_cmp(&(*r as f64)),
        (Value::String(l), Value::String(r)) => Some(compare_strings_with_temporal(l, r)),
        _ => left.partial_cmp(right),
    }
}

fn compare_strings_with_temporal(left: &str, right: &str) -> Ordering {
    match (parse_temporal_string(left), parse_temporal_string(right)) {
        (Some(TemporalValue::Date(l)), Some(TemporalValue::Date(r))) => l.cmp(&r),
        (Some(TemporalValue::LocalTime(l)), Some(TemporalValue::LocalTime(r))) => {
            compare_time_of_day(l, r)
        }
        (
            Some(TemporalValue::Time {
                time: lt,
                offset: lo,
            }),
            Some(TemporalValue::Time {
                time: rt,
                offset: ro,
            }),
        ) => compare_time_with_offset(lt, lo, rt, ro),
        (Some(TemporalValue::LocalDateTime(l)), Some(TemporalValue::LocalDateTime(r))) => l.cmp(&r),
        (Some(TemporalValue::DateTime(l)), Some(TemporalValue::DateTime(r))) => l.cmp(&r),
        _ => left.cmp(right),
    }
}
