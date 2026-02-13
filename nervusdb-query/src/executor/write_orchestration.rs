use super::{
    GraphSnapshot, MergeOverlayState, Plan, Result, Row, WriteableGraph,
    apply_label_overlay_to_rows, apply_removed_property_overlay_to_rows,
    apply_set_map_overlay_to_rows, apply_set_property_overlay_to_rows, execute_create_write_rows,
    execute_delete_on_rows, execute_foreach, execute_merge_create_from_rows, execute_plan,
    execute_remove, execute_remove_labels, execute_set, execute_set_from_maps, execute_set_labels,
};
use crate::ast::Expression;

pub(super) fn execute_write_with_rows<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
) -> Result<(u32, Vec<Row>)> {
    match plan {
        Plan::Create { .. } | Plan::Delete { .. } => {
            execute_create_write_rows(plan, snapshot, txn, params)
        }
        Plan::SetProperty { input, items } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_set_property_overlay_to_rows(snapshot, rows, items, params);
            Ok((prefix_mods + updated, rows))
        }
        Plan::SetPropertiesFromMap { input, items } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set_from_maps(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_set_map_overlay_to_rows(snapshot, rows, items, params);
            Ok((prefix_mods + updated, rows))
        }
        Plan::SetLabels { input, items } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set_labels(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_label_overlay_to_rows(snapshot, rows, items, true);
            Ok((prefix_mods + updated, rows))
        }
        Plan::RemoveProperty { input, items } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let removed = execute_remove(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_removed_property_overlay_to_rows(snapshot, rows, items);
            Ok((prefix_mods + removed, rows))
        }
        Plan::RemoveLabels { input, items } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let removed = execute_remove_labels(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_label_overlay_to_rows(snapshot, rows, items, false);
            Ok((prefix_mods + removed, rows))
        }
        Plan::Foreach {
            input,
            variable,
            list,
            sub_plan,
        } => {
            let (prefix_mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let changed = execute_foreach(
                snapshot,
                &values_plan,
                txn,
                variable,
                list,
                sub_plan,
                params,
            )?;
            Ok((prefix_mods + changed, rows))
        }
        Plan::Filter { input, predicate } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Filter {
                input: Box::new(Plan::Values { rows }),
                predicate: predicate.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Project { input, projections } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Project {
                input: Box::new(Plan::Values { rows }),
                projections: projections.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Aggregate {
            input,
            group_by,
            aggregates,
        } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Aggregate {
                input: Box::new(Plan::Values { rows }),
                group_by: group_by.clone(),
                aggregates: aggregates.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::OrderBy { input, items } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::OrderBy {
                input: Box::new(Plan::Values { rows }),
                items: items.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Skip { input, skip } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Skip {
                input: Box::new(Plan::Values { rows }),
                skip: *skip,
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Limit { input, limit } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Limit {
                input: Box::new(Plan::Values { rows }),
                limit: *limit,
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Distinct { input } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Distinct {
                input: Box::new(Plan::Values { rows }),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Unwind {
            input,
            expression,
            alias,
        } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Unwind {
                input: Box::new(Plan::Values { rows }),
                expression: expression.clone(),
                alias: alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::ProcedureCall {
            input,
            name,
            args,
            yields,
        } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::ProcedureCall {
                input: Box::new(Plan::Values { rows }),
                name: name.clone(),
                args: args.clone(),
                yields: yields.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::MatchOut {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            project,
            project_external,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_write_with_rows(inner, snapshot, txn, params)?;
                let staged = Plan::MatchOut {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    project: project.clone(),
                    project_external: *project_external,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchOutVarLen {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            direction,
            min_hops,
            max_hops,
            limit,
            project,
            project_external,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_write_with_rows(inner, snapshot, txn, params)?;
                let staged = Plan::MatchOutVarLen {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    direction: direction.clone(),
                    min_hops: *min_hops,
                    max_hops: *max_hops,
                    limit: *limit,
                    project: project.clone(),
                    project_external: *project_external,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchIn {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_write_with_rows(inner, snapshot, txn, params)?;
                let staged = Plan::MatchIn {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchUndirected {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_write_with_rows(inner, snapshot, txn, params)?;
                let staged = Plan::MatchUndirected {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchBoundRel {
            input,
            rel_alias,
            src_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            rels,
            direction,
            optional,
            optional_unbind,
            path_alias,
        } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::MatchBoundRel {
                input: Box::new(Plan::Values { rows }),
                rel_alias: rel_alias.clone(),
                src_alias: src_alias.clone(),
                dst_alias: dst_alias.clone(),
                dst_labels: dst_labels.clone(),
                src_prebound: *src_prebound,
                rels: rels.clone(),
                direction: direction.clone(),
                optional: *optional,
                optional_unbind: optional_unbind.clone(),
                path_alias: path_alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Apply {
            input,
            subquery,
            alias,
        } => {
            let (mods, rows) = execute_write_with_rows(input, snapshot, txn, params)?;
            let staged = Plan::Apply {
                input: Box::new(Plan::Values { rows }),
                subquery: subquery.clone(),
                alias: alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        _ => {
            let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
            Ok((0, out_rows))
        }
    }
}

pub(super) fn execute_merge_with_rows<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
    on_create_items: &[(String, String, Expression)],
    on_match_items: &[(String, String, Expression)],
) -> Result<(u32, Vec<Row>)> {
    let mut overlay = MergeOverlayState::default();
    execute_merge_with_rows_inner(
        plan,
        snapshot,
        txn,
        params,
        on_create_items,
        on_match_items,
        &mut overlay,
    )
}

fn execute_merge_with_rows_inner<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
    on_create_items: &[(String, String, Expression)],
    on_match_items: &[(String, String, Expression)],
    overlay: &mut MergeOverlayState,
) -> Result<(u32, Vec<Row>)> {
    match plan {
        Plan::Create { input, pattern } => {
            let (prefix_mods, input_rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let (created, out_rows) = execute_merge_create_from_rows(
                snapshot,
                input_rows,
                txn,
                pattern,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            Ok((prefix_mods + created, out_rows))
        }
        Plan::Delete {
            input,
            detach,
            expressions,
        } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let deleted =
                execute_delete_on_rows(snapshot, &rows, txn, *detach, expressions, params)?;
            Ok((prefix_mods + deleted, rows))
        }
        Plan::SetProperty { input, items } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_set_property_overlay_to_rows(snapshot, rows, items, params);
            Ok((prefix_mods + updated, rows))
        }
        Plan::SetPropertiesFromMap { input, items } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set_from_maps(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_set_map_overlay_to_rows(snapshot, rows, items, params);
            Ok((prefix_mods + updated, rows))
        }
        Plan::SetLabels { input, items } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let updated = execute_set_labels(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_label_overlay_to_rows(snapshot, rows, items, true);
            Ok((prefix_mods + updated, rows))
        }
        Plan::RemoveProperty { input, items } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let removed = execute_remove(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_removed_property_overlay_to_rows(snapshot, rows, items);
            Ok((prefix_mods + removed, rows))
        }
        Plan::RemoveLabels { input, items } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let removed = execute_remove_labels(snapshot, &values_plan, txn, items, params)?;
            let rows = apply_label_overlay_to_rows(snapshot, rows, items, false);
            Ok((prefix_mods + removed, rows))
        }
        Plan::Foreach {
            input,
            variable,
            list,
            sub_plan,
        } => {
            let (prefix_mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let values_plan = Plan::Values { rows: rows.clone() };
            let changed = execute_foreach(
                snapshot,
                &values_plan,
                txn,
                variable,
                list,
                sub_plan,
                params,
            )?;
            Ok((prefix_mods + changed, rows))
        }
        Plan::Filter { input, predicate } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Filter {
                input: Box::new(Plan::Values { rows }),
                predicate: predicate.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Project { input, projections } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Project {
                input: Box::new(Plan::Values { rows }),
                projections: projections.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Aggregate {
            input,
            group_by,
            aggregates,
        } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Aggregate {
                input: Box::new(Plan::Values { rows }),
                group_by: group_by.clone(),
                aggregates: aggregates.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::OrderBy { input, items } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::OrderBy {
                input: Box::new(Plan::Values { rows }),
                items: items.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Skip { input, skip } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Skip {
                input: Box::new(Plan::Values { rows }),
                skip: *skip,
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Limit { input, limit } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Limit {
                input: Box::new(Plan::Values { rows }),
                limit: *limit,
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Distinct { input } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Distinct {
                input: Box::new(Plan::Values { rows }),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Unwind {
            input,
            expression,
            alias,
        } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Unwind {
                input: Box::new(Plan::Values { rows }),
                expression: expression.clone(),
                alias: alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::ProcedureCall {
            input,
            name,
            args,
            yields,
        } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::ProcedureCall {
                input: Box::new(Plan::Values { rows }),
                name: name.clone(),
                args: args.clone(),
                yields: yields.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::MatchOut {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            project,
            project_external,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_merge_with_rows_inner(
                    inner,
                    snapshot,
                    txn,
                    params,
                    on_create_items,
                    on_match_items,
                    overlay,
                )?;
                let staged = Plan::MatchOut {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    project: project.clone(),
                    project_external: *project_external,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchOutVarLen {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            direction,
            min_hops,
            max_hops,
            limit,
            project,
            project_external,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_merge_with_rows_inner(
                    inner,
                    snapshot,
                    txn,
                    params,
                    on_create_items,
                    on_match_items,
                    overlay,
                )?;
                let staged = Plan::MatchOutVarLen {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    direction: direction.clone(),
                    min_hops: *min_hops,
                    max_hops: *max_hops,
                    limit: *limit,
                    project: project.clone(),
                    project_external: *project_external,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchIn {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_merge_with_rows_inner(
                    inner,
                    snapshot,
                    txn,
                    params,
                    on_create_items,
                    on_match_items,
                    overlay,
                )?;
                let staged = Plan::MatchIn {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchUndirected {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            optional,
            optional_unbind,
            path_alias,
        } => {
            if let Some(inner) = input {
                let (mods, rows) = execute_merge_with_rows_inner(
                    inner,
                    snapshot,
                    txn,
                    params,
                    on_create_items,
                    on_match_items,
                    overlay,
                )?;
                let staged = Plan::MatchUndirected {
                    input: Some(Box::new(Plan::Values { rows })),
                    src_alias: src_alias.clone(),
                    rels: rels.clone(),
                    edge_alias: edge_alias.clone(),
                    dst_alias: dst_alias.clone(),
                    dst_labels: dst_labels.clone(),
                    src_prebound: *src_prebound,
                    limit: *limit,
                    optional: *optional,
                    optional_unbind: optional_unbind.clone(),
                    path_alias: path_alias.clone(),
                };
                let out_rows =
                    execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
                Ok((mods, out_rows))
            } else {
                let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
                Ok((0, out_rows))
            }
        }
        Plan::MatchBoundRel {
            input,
            rel_alias,
            src_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            rels,
            direction,
            optional,
            optional_unbind,
            path_alias,
        } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::MatchBoundRel {
                input: Box::new(Plan::Values { rows }),
                rel_alias: rel_alias.clone(),
                src_alias: src_alias.clone(),
                dst_alias: dst_alias.clone(),
                dst_labels: dst_labels.clone(),
                src_prebound: *src_prebound,
                rels: rels.clone(),
                direction: direction.clone(),
                optional: *optional,
                optional_unbind: optional_unbind.clone(),
                path_alias: path_alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        Plan::Apply {
            input,
            subquery,
            alias,
        } => {
            let (mods, rows) = execute_merge_with_rows_inner(
                input,
                snapshot,
                txn,
                params,
                on_create_items,
                on_match_items,
                overlay,
            )?;
            let staged = Plan::Apply {
                input: Box::new(Plan::Values { rows }),
                subquery: subquery.clone(),
                alias: alias.clone(),
            };
            let out_rows = execute_plan(snapshot, &staged, params).collect::<Result<Vec<_>>>()?;
            Ok((mods, out_rows))
        }
        _ => {
            let out_rows = execute_plan(snapshot, plan, params).collect::<Result<Vec<_>>>()?;
            Ok((0, out_rows))
        }
    }
}
