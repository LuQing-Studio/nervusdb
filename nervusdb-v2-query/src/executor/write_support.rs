use super::{PropertyValue, Row, WriteableGraph, convert_executor_value_to_property};
use crate::ast::Expression;
use crate::error::{Error, Result};
use crate::evaluator::evaluate_expression_value;
use nervusdb_v2_api::GraphSnapshot;

pub(super) fn merge_apply_set_items<S: GraphSnapshot>(
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    row: &Row,
    items: &[(String, String, Expression)],
    params: &crate::query_api::Params,
) -> Result<()> {
    for (var, key, expr) in items {
        let val = evaluate_expression_value(expr, row, snapshot, params);
        let prop_val = convert_executor_value_to_property(&val)?;
        let is_remove = matches!(prop_val, PropertyValue::Null);
        if let Some(node_id) = row.get_node(var) {
            if is_remove {
                txn.remove_node_property(node_id, key)?;
            } else {
                txn.set_node_property(node_id, key.clone(), prop_val)?;
            }
        } else if let Some(edge) = row.get_edge(var) {
            if is_remove {
                txn.remove_edge_property(edge.src, edge.rel, edge.dst, key)?;
            } else {
                txn.set_edge_property(edge.src, edge.rel, edge.dst, key.clone(), prop_val)?;
            }
        } else {
            return Err(Error::Other(format!("Variable {} not found in row", var)));
        }
    }
    Ok(())
}

pub(super) fn merge_eval_props_on_row<S: GraphSnapshot>(
    snapshot: &S,
    row: &Row,
    props: &Option<crate::ast::PropertyMap>,
    params: &crate::query_api::Params,
) -> Result<std::collections::BTreeMap<String, PropertyValue>> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(props) = props {
        for pair in &props.properties {
            let v = evaluate_expression_value(&pair.value, row, snapshot, params);
            out.insert(pair.key.clone(), convert_executor_value_to_property(&v)?);
        }
    }
    Ok(out)
}
