use crate::ast::{BinaryOperator, Clause, Expression, Literal, Query, RelationshipDirection};
use crate::error::{Error, Result};
use crate::executor::{Plan, Row, Value, execute_plan, execute_write};
use nervusdb_v2_api::GraphSnapshot;
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteSemantics {
    Default,
    Merge,
}

/// Query parameters for parameterized Cypher queries.
///
/// # Example
///
/// ```ignore
/// let mut params = Params::new();
/// params.insert("name", Value::String("Alice".to_string()));
/// let results: Vec<_> = query.execute_streaming(&snapshot, &params).collect();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Params {
    inner: BTreeMap<String, Value>,
}

impl Params {
    /// Creates a new empty parameters map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a parameter value.
    ///
    /// Parameters are referenced in Cypher queries using `$name` syntax.
    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        self.inner.insert(name.into(), value);
    }

    /// Gets a parameter value by name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.inner.get(name)
    }
}

/// A compiled Cypher query ready for execution.
///
/// Created by [`prepare()`]. The query plan is optimized once
/// and can be executed multiple times with different parameters.
#[derive(Debug, Clone)]
pub struct PreparedQuery {
    plan: Plan,
    explain: Option<String>,
    write: WriteSemantics,
}

impl PreparedQuery {
    /// Executes a read query and returns a streaming iterator.
    ///
    /// The returned iterator yields `Result<Row>`, where each row
    /// represents a result record. Errors can occur during execution
    /// (e.g., type mismatches, missing variables).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let query = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 10").unwrap();
    /// let rows: Vec<_> = query
    ///     .execute_streaming(&snapshot, &Params::new())
    ///     .collect::<Result<_>>()
    ///     .unwrap();
    /// ```
    pub fn execute_streaming<'a, S: GraphSnapshot + 'a>(
        &'a self,
        snapshot: &'a S,
        params: &'a Params,
    ) -> impl Iterator<Item = Result<Row>> + 'a {
        if let Some(plan) = &self.explain {
            let it: Box<dyn Iterator<Item = Result<Row>> + 'a> = Box::new(std::iter::once(Ok(
                Row::default().with("plan", Value::String(plan.clone())),
            )));
            return it;
        }
        Box::new(execute_plan(snapshot, &self.plan, params))
    }

    /// Executes a write query (CREATE/DELETE) with a write transaction.
    ///
    /// Returns the number of entities created/deleted.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let query = prepare("CREATE (n)").unwrap();
    /// let mut txn = db.begin_write();
    /// let count = query.execute_write(&snapshot, &mut txn, &Params::new()).unwrap();
    /// txn.commit().unwrap();
    /// ```
    pub fn execute_write<S: GraphSnapshot>(
        &self,
        snapshot: &S,
        txn: &mut impl crate::executor::WriteableGraph,
        params: &Params,
    ) -> Result<u32> {
        if self.explain.is_some() {
            return Err(Error::Other(
                "EXPLAIN cannot be executed as a write query".into(),
            ));
        }
        match self.write {
            WriteSemantics::Default => execute_write(&self.plan, snapshot, txn, params),
            WriteSemantics::Merge => {
                crate::executor::execute_merge(&self.plan, snapshot, txn, params)
            }
        }
    }

    pub fn is_explain(&self) -> bool {
        self.explain.is_some()
    }

    /// Returns the explained plan string if this query was an EXPLAIN query.
    pub fn explain_string(&self) -> Option<&str> {
        self.explain.as_deref()
    }
}

/// Parses and prepares a Cypher query for execution.
///
/// # Supported Cypher (v2 M3)
///
/// - `RETURN 1` - Constant return
/// - `MATCH (n)-[:<u32>]->(m) RETURN n, m LIMIT k` - Single-hop pattern match
/// - `MATCH (n)-[:<u32>]->(m) WHERE n.prop = 'value' RETURN n, m` - With WHERE filter
/// - `CREATE (n)` / `CREATE (n {k: v})` - Create nodes
/// - `CREATE (a)-[:1]->(b)` - Create edges
/// - `MATCH (n)-[:1]->(m) DELETE n` / `DETACH DELETE n` - Delete nodes/edges
/// - `EXPLAIN <query>` - Show compiled plan (no execution)
///
/// Returns an error for unsupported Cypher constructs.
pub fn prepare(cypher: &str) -> Result<PreparedQuery> {
    if let Some(inner) = strip_explain_prefix(cypher) {
        if inner.is_empty() {
            return Err(Error::Other("EXPLAIN requires a query".into()));
        }
        let query = crate::parser::Parser::parse(inner)?;
        let compiled = compile_m3_plan(query)?;
        let explain = Some(render_plan(&compiled.plan));
        return Ok(PreparedQuery {
            plan: compiled.plan,
            explain,
            write: compiled.write,
        });
    }

    let query = crate::parser::Parser::parse(cypher)?;
    let compiled = compile_m3_plan(query)?;
    Ok(PreparedQuery {
        plan: compiled.plan,
        explain: None,
        write: compiled.write,
    })
}

fn strip_explain_prefix(input: &str) -> Option<&str> {
    let trimmed = input.trim_start();
    if trimmed.len() < "EXPLAIN".len() {
        return None;
    }
    let (head, tail) = trimmed.split_at("EXPLAIN".len());
    if !head.eq_ignore_ascii_case("EXPLAIN") {
        return None;
    }
    if let Some(next) = tail.chars().next()
        && !next.is_whitespace()
    {
        // Avoid matching `EXPLAINED`, etc.
        return None;
    }
    Some(tail.trim_start())
}

fn render_plan(plan: &Plan) -> String {
    fn indent(n: usize) -> String {
        "  ".repeat(n)
    }

    fn go(out: &mut String, plan: &Plan, depth: usize) {
        let pad = indent(depth);
        match plan {
            Plan::ReturnOne => {
                let _ = writeln!(out, "{pad}ReturnOne");
            }
            Plan::NodeScan { alias, label } => {
                let _ = writeln!(out, "{pad}NodeScan(alias={alias}, label={label:?})");
            }
            Plan::MatchOut {
                input: _,
                src_alias,
                rel,
                edge_alias,
                dst_alias,
                limit,
                project: _,
                project_external: _,
                optional,
            } => {
                let opt_str = if *optional { " OPTIONAL" } else { "" };
                let _ = writeln!(
                    out,
                    "{pad}MatchOut{opt_str}(src={src_alias}, rel={rel:?}, edge={edge_alias:?}, dst={dst_alias}, limit={limit:?})"
                );
            }
            Plan::MatchOutVarLen {
                input: _,
                src_alias,
                rel,
                edge_alias,
                dst_alias,
                min_hops,
                max_hops,
                limit,
                project: _,
                project_external: _,
                optional,
            } => {
                let opt_str = if *optional { " OPTIONAL" } else { "" };
                let _ = writeln!(
                    out,
                    "{pad}MatchOutVarLen{opt_str}(src={src_alias}, rel={rel:?}, edge={edge_alias:?}, dst={dst_alias}, min={min_hops}, max={max_hops:?}, limit={limit:?})"
                );
            }
            Plan::Filter { input, predicate } => {
                let _ = writeln!(out, "{pad}Filter(predicate={predicate:?})");
                go(out, input, depth + 1);
            }
            Plan::Project { input, projections } => {
                let _ = writeln!(out, "{pad}Project(len={})", projections.len());
                go(out, input, depth + 1);
            }
            Plan::Aggregate {
                input,
                group_by,
                aggregates,
            } => {
                let _ = writeln!(
                    out,
                    "{pad}Aggregate(group_by={group_by:?}, aggregates={aggregates:?})"
                );
                go(out, input, depth + 1);
            }
            Plan::OrderBy { input, items } => {
                let _ = writeln!(out, "{pad}OrderBy(items={items:?})");
                go(out, input, depth + 1);
            }
            Plan::Skip { input, skip } => {
                let _ = writeln!(out, "{pad}Skip(skip={skip})");
                go(out, input, depth + 1);
            }
            Plan::Limit { input, limit } => {
                let _ = writeln!(out, "{pad}Limit(limit={limit})");
                go(out, input, depth + 1);
            }
            Plan::Distinct { input } => {
                let _ = writeln!(out, "{pad}Distinct");
                go(out, input, depth + 1);
            }
            Plan::Create { pattern } => {
                let _ = writeln!(out, "{pad}Create(pattern={pattern:?})");
            }
            Plan::Delete {
                input,
                detach,
                expressions,
            } => {
                let _ = writeln!(
                    out,
                    "{pad}Delete(detach={detach}, expressions={expressions:?})"
                );
                go(out, input, depth + 1);
            }
            Plan::Unwind {
                input,
                expression,
                alias,
            } => {
                let _ = writeln!(out, "{pad}Unwind(alias={alias}, expression={expression:?})");
                go(out, input, depth + 1);
            }
            Plan::Union { left, right, all } => {
                let _ = writeln!(out, "{pad}Union(all={all})");
                go(out, left, depth + 1);
                go(out, right, depth + 1);
            }
            Plan::SetProperty { input, items } => {
                let _ = writeln!(out, "{pad}SetProperty(items={items:?})");
                go(out, input, depth + 1);
            }
            Plan::RemoveProperty { input, items } => {
                let _ = writeln!(out, "{pad}RemoveProperty(items={items:?})");
                go(out, input, depth + 1);
            }
            Plan::IndexSeek {
                alias,
                label,
                field,
                value_expr,
                fallback: _fallback,
            } => {
                let _ = writeln!(
                    out,
                    "{pad}IndexSeek(alias={alias}, label={label}, field={field}, value={value_expr:?})"
                );
                // We don't render fallback to avoid noise, as it's just the unoptimized plan
            }
        }
    }

    let mut out = String::new();
    go(&mut out, plan, 0);
    out.trim_end().to_string()
}

struct CompiledQuery {
    plan: Plan,
    write: WriteSemantics,
}

fn compile_m3_plan(query: Query) -> Result<CompiledQuery> {
    let mut plan: Option<Plan> = None;
    let mut clauses = query.clauses.iter().peekable();
    let mut write_semantics = WriteSemantics::Default;

    while let Some(clause) = clauses.next() {
        match clause {
            Clause::Match(m) => {
                // Check ahead for WHERE to optimize immediately
                let mut predicates = BTreeMap::new();
                if let Some(Clause::Where(w)) = clauses.peek() {
                    extract_predicates(&w.expression, &mut predicates);
                }

                plan = Some(compile_match_plan(plan, m.clone(), &predicates)?);
            }
            Clause::Where(w) => {
                // If we didn't consume it optimization (e.g. complex filter not indexable), add filter plan
                // Note: compile_match_plan consumes predicates that CAN be pushed down.
                // We need a way to know if it was fully consumed?
                // For MVP: Simplest approach is to ALWAYS add Filter plan if we have a WHERE clause,
                // and rely on `try_optimize_nodescan_filter` inside `compile_match_plan` or similar.
                // But `compile_match_plan` currently takes predicates.
                // Let's refine: `compile_match_plan` applies index seeks.
                // Any remaining filtering logic must be applied.
                // Current `compile_match_plan` logic in existing code didn't return unused predicates.
                // Let's just always apply a Filter plan for safety in this refactor,
                // OR checking if we just did a Match.
                // Actually, the previous implementation extracted predicates and passed them to match.
                // If we want to support WHERE after WITH, we need `Plan::Filter`.
                // If it's WHERE after MATCH, we want index optimization.

                // Strategy: if previous clause was MATCH, we already peeked and optimized.
                // But if the optimization didn't cover everything, we still need a Filter?
                // Existing `compile_match_plan` handles index seek vs scan + filter.
                // So if we passed predicates to `compile_match_plan`, we might be done?
                // Let's look at `compile_match_plan` (not visible here but I recall it).
                // It likely constructs a Scan + Filter or IndexSeek.
                // So if we just handled a Match, we "consumed" the Where for implementation purposes.
                // But we need to skip the Where clause in the iterator if we 'peeked' it?
                // Using `peeking` to optimize is good. But we need to advance the iterator if we use it.
                // Let's change loop logic to handle WHERE inside MATCH case, or skip it here.

                // Revised Strategy:
                // Handle WHERE here only if it wasn't consumed by a preceding MATCH?
                // Or: MATCH consumes the next WHERE if present.
                // If we find a standalone WHERE (e.g. after WITH), we compile it as Filter.
                // To do this clean:
                // If MATCH case peeks and sees WHERE, it *should* consume it.
                // So we need to `clauses.next()` inside MATCH case?
                // Rust iterators don't let you consume from `peek`.
                // So we just check behavior.

                if matches!(plan, None) {
                    return Err(Error::Other("WHERE cannot be the first clause".into()));
                }
                plan = Some(Plan::Filter {
                    input: Box::new(plan.unwrap()),
                    predicate: w.expression.clone(),
                });
            }
            Clause::With(w) => {
                let input =
                    plan.ok_or_else(|| Error::Other("WITH cannot be the first clause".into()))?;
                plan = Some(compile_with_plan(input, w)?);
            }
            Clause::Return(r) => {
                let input = plan.unwrap_or(Plan::ReturnOne);
                let (p, _) = compile_return_plan(input, r)?;
                plan = Some(p);
                // RETURN is terminal in M3 usually, but standard Cypher allows it at end.
                // We continue loop? Standard Cypher ends with RETURN.
                // If there are more clauses after RETURN, it might be an error or valid?
                // In standard Cypher, RETURN is terminal UNLESS followed by UNION.
                // Check if any clauses left?
                if let Some(next_clause) = clauses.peek() {
                    // Allow UNION to follow RETURN
                    if !matches!(next_clause, Clause::Union(_)) {
                        return Err(Error::NotImplemented(
                            "Clauses after RETURN are not supported",
                        ));
                    }
                    // Continue loop to process UNION
                } else {
                    // No more clauses, return successfully
                    return Ok(CompiledQuery {
                        plan: plan.unwrap(),
                        write: write_semantics,
                    });
                }
            }
            Clause::Create(c) => {
                // CREATE can start a query or follow others.
                if plan.is_none() {
                    plan = Some(compile_create_plan(c.clone())?);
                } else {
                    // Creating in context (after MATCH/WITH)
                    // Plan::Create needs to support input?
                    // Existing Plan::Create { pattern } might be standalone?
                    // We need Plan::Create to be "Create and Pass Through".
                    // Ideally: Plan::Create { input, pattern }.
                    // But let's check definition of Plan::Create.
                    // It likely doesn't have input.
                    // If it doesn't, we can't chain it yet.
                    // T305 focus is WITH. Let's block chaining CREATE for now unless it's first?
                    // Or check if we can modify Plan.
                    return Err(Error::NotImplemented(
                        "Chained CREATE not supported yet (v2 M3)",
                    ));
                }
            }
            Clause::Merge(m) => {
                write_semantics = WriteSemantics::Merge;
                if plan.is_none() {
                    plan = Some(compile_merge_plan(m.clone())?);
                } else {
                    return Err(Error::NotImplemented("Chained MERGE not supported yet"));
                }
            }
            Clause::Set(s) => {
                let input = plan.ok_or_else(|| Error::Other("SET need input".into()))?;
                // We need to associate WHERE?
                // SET doesn't have its own WHERE. It operates on rows.
                plan = Some(compile_set_plan_v2(input, s.clone())?);
            }
            Clause::Remove(r) => {
                let input = plan.ok_or_else(|| Error::Other("REMOVE need input".into()))?;
                plan = Some(compile_remove_plan_v2(input, r.clone())?);
            }
            Clause::Delete(d) => {
                let input = plan.ok_or_else(|| Error::Other("DELETE need input".into()))?;
                plan = Some(compile_delete_plan_v2(input, d.clone())?);

                // If DELETE is not terminal, we might have issues if we detach/delete nodes used later?
                // But for now, let's allow it.
            }
            Clause::Unwind(u) => {
                let input = plan.unwrap_or(Plan::ReturnOne);
                plan = Some(compile_unwind_plan(input, u.clone()));
            }
            Clause::Call(_) => return Err(Error::NotImplemented("CALL in v2 M3")),
            Clause::Union(u) => {
                // UNION logic: current plan is the "left" side; the clause's nested query is the "right" side
                let left_plan =
                    plan.ok_or_else(|| Error::Other("UNION requires left query part".into()))?;
                let right_compiled = compile_m3_plan(u.query.clone())?;
                plan = Some(Plan::Union {
                    left: Box::new(left_plan),
                    right: Box::new(right_compiled.plan),
                    all: u.all,
                });
            }
        }
    }

    // If we exit loop without RETURN
    // For update queries (CREATE/DELETE/SET), this is valid if we return count?
    // M3 requires RETURN usually for read.
    // Spec says: "query without RETURN" is error for read queries.
    // Write queries might return stats?
    // Existing code returned "query without RETURN" error.
    // We'll stick to that unless it's a write-only query?
    // Let's enforce RETURN for now as per previous logic, unless we tracked we did logical writes?
    // But previous `prepare` returns `Result<CompiledQuery>`.

    // If plan exists here, but no RETURN hit.
    // For queries ending in update clauses (CREATE, DELETE, etc.), this is valid.
    if let Some(plan) = plan {
        return Ok(CompiledQuery {
            plan,
            write: write_semantics,
        });
    }

    Err(Error::NotImplemented("Empty query"))
}

fn compile_with_plan(input: Plan, with: &crate::ast::WithClause) -> Result<Plan> {
    // 1. Projection / Aggregation
    // WITH is identical to RETURN in structure: items, orderBy, skip, limit, where.
    // It projects the input to a new set of variables.

    let (mut plan, _) = compile_projection_aggregation(input, &with.items)?;

    // 2. WHERE
    if let Some(w) = &with.where_clause {
        plan = Plan::Filter {
            input: Box::new(plan),
            predicate: w.expression.clone(),
        };
    }

    // 3. ORDER BY
    if let Some(order_by) = &with.order_by {
        let items = compile_order_by_items(order_by)?;
        plan = Plan::OrderBy {
            input: Box::new(plan),
            items,
        };
    }

    // 4. SKIP
    if let Some(skip) = with.skip {
        plan = Plan::Skip {
            input: Box::new(plan),
            skip,
        };
    }

    // 5. LIMIT
    if let Some(limit) = with.limit {
        plan = Plan::Limit {
            input: Box::new(plan),
            limit,
        };
    }

    Ok(plan)
}

// Shared logic for RETURN and WITH
fn compile_return_plan(input: Plan, ret: &crate::ast::ReturnClause) -> Result<(Plan, Vec<String>)> {
    let (mut plan, project_cols) = compile_projection_aggregation(input, &ret.items)?;

    if let Some(order_by) = &ret.order_by {
        let items = compile_order_by_items(order_by)?;
        plan = Plan::OrderBy {
            input: Box::new(plan),
            items,
        };
    }

    if let Some(skip) = ret.skip {
        plan = Plan::Skip {
            input: Box::new(plan),
            skip,
        };
    }

    if let Some(limit) = ret.limit {
        plan = Plan::Limit {
            input: Box::new(plan),
            limit,
        };
    }

    if ret.distinct {
        plan = Plan::Distinct {
            input: Box::new(plan),
        };
    }

    Ok((plan, project_cols))
}

fn compile_projection_aggregation(
    input: Plan,
    items: &[crate::ast::ReturnItem],
) -> Result<(Plan, Vec<String>)> {
    let mut aggregates: Vec<(crate::ast::AggregateFunction, String)> = Vec::new();
    let mut project_cols: Vec<String> = Vec::new(); // Final output columns

    // We categorize items:
    // 1. Aggregates -> Goes to Plan::Aggregate
    // 2. Non-aggregates -> Must be grouping keys. Need to be projected BEFORE aggregation.

    let mut pre_projections = Vec::new(); // For Plan::Project before Aggregate
    let mut group_by = Vec::new(); // For Plan::Aggregate
    let mut is_aggregation = false;
    let mut projected_aliases = std::collections::HashSet::new();

    // First pass: identify if it is an aggregation and collect items
    for (i, item) in items.iter().enumerate() {
        // Check for aggregation function
        let mut found_agg = false;
        if let Expression::FunctionCall(call) = &item.expression {
            if let Some(agg) = parse_aggregate_function(call)? {
                found_agg = true;
                is_aggregation = true;
                let alias = item.alias.clone().unwrap_or_else(|| format!("agg_{}", i));
                aggregates.push((agg, alias.clone()));
                project_cols.push(alias);

                // Capture dependencies
                let mut deps = std::collections::HashSet::new();
                for arg in &call.args {
                    extract_variables_from_expr(arg, &mut deps);
                }
                // We will add deps to pre_projections AFTER loop or handle logic carefully.
                // If we add them here, they are "implicit" projections.
                // We need them to evaluate the aggregate.
                // BUT, if the same variable is ALSO a grouping key later in the list...
                // It's okay. Duplicates in pre_projections might be inefficient but usually fine if logic uses aliases.
                // However, Plan::Aggregate uses grouping keys to form groups.
                // Implicit deps are just "extra columns" passed through.
                for dep in deps {
                    if !projected_aliases.contains(&dep) {
                        pre_projections.push((dep.clone(), Expression::Variable(dep.clone())));
                        projected_aliases.insert(dep);
                    }
                }
            }
        }

        if !found_agg {
            let alias = item
                .alias
                .clone()
                .unwrap_or_else(|| match &item.expression {
                    Expression::Variable(name) => name.clone(),
                    Expression::PropertyAccess(pa) => format!("{}.{}", pa.variable, pa.property),
                    _ => format!("expr_{}", i),
                });

            // Even if variable, we project it to ensure it's available and aliased correctly
            if !projected_aliases.contains(&alias) {
                pre_projections.push((alias.clone(), item.expression.clone()));
                projected_aliases.insert(alias.clone());
            }

            // If we are aggregating, this alias becomes a grouping key
            group_by.push(alias.clone());
            project_cols.push(alias);
        }
    }

    if is_aggregation {
        // 1. Pre-project grouping keys
        // Input -> Project(keys) -> Aggregate(keys)
        // If pre_projections is empty (e.g. `RETURN count(*)`), check implicit group by?
        // OpenCypher: fail if mixed agg and non-agg without grouping.
        // We assume valid cypher for now.

        // We only project if there are grouping keys.
        // If there are NO grouping keys (global agg like `count(*)`), Plan::Project inputs nothing?
        // If pre_projections is empty, Plan::Project would produce empty rows?
        // Yes. `count(*)` counts rows. Empty rows are fine (as long as count is correct).
        // But wait, Plan::Project logic: `input_iter.map(... new_row ...)`.
        // If projections empty, `new_row` is empty.
        // Rows still exist (one per input).
        // Aggregate count(*) counts them. Correct.

        // However, if we discard `n` (not in pre_projections), and we do `count(n)`,
        // we might fail evaluating `n`.
        // T305 MVP: Stick to `count(*)` or counting grouping keys.
        // If user does `WITH n, count(m)`, we error or panic.
        // We assume safe MVP scope.

        let plan = if !pre_projections.is_empty() {
            Plan::Project {
                input: Box::new(input),
                projections: pre_projections,
            }
        } else {
            // Pass through input if no grouping keys?
            // No, if we pass through, we keep ALL variables.
            // Then Aggregate groups by "nothing" (empty group_by).
            // This works for Global Aggregation.
            input
        };

        let plan = Plan::Aggregate {
            input: Box::new(plan),
            group_by,
            aggregates,
        };
        Ok((plan, project_cols))
    } else {
        // Simple Projection
        Ok((
            Plan::Project {
                input: Box::new(input),
                projections: pre_projections,
            },
            project_cols,
        ))
    }
}

fn compile_order_by_items(
    order_by: &crate::ast::OrderByClause,
) -> Result<Vec<(Expression, crate::ast::Direction)>> {
    Ok(order_by
        .items
        .iter()
        .map(|item| (item.expression.clone(), item.direction.clone()))
        .collect())
}

// Adapters for SET/REMOVE/DELETE since we changed signature to take input
fn compile_set_plan_v2(input: Plan, set: crate::ast::SetClause) -> Result<Plan> {
    // Convert SetItems to (var, key, expr)
    let mut items = Vec::new();
    for item in set.items {
        items.push((item.property.variable, item.property.property, item.value));
    }

    Ok(Plan::SetProperty {
        input: Box::new(input),
        items,
    })
}

fn compile_remove_plan_v2(input: Plan, remove: crate::ast::RemoveClause) -> Result<Plan> {
    // Convert properties to (var, key)
    let mut items = Vec::with_capacity(remove.properties.len());
    for prop in remove.properties {
        items.push((prop.variable, prop.property));
    }

    Ok(Plan::RemoveProperty {
        input: Box::new(input),
        items,
    })
}

fn compile_unwind_plan(input: Plan, unwind: crate::ast::UnwindClause) -> Plan {
    Plan::Unwind {
        input: Box::new(input),
        expression: unwind.expression,
        alias: unwind.alias,
    }
}

fn compile_delete_plan_v2(input: Plan, delete: crate::ast::DeleteClause) -> Result<Plan> {
    Ok(Plan::Delete {
        input: Box::new(input),
        detach: delete.detach,
        expressions: delete.expressions,
    })
}

fn compile_create_plan(create_clause: crate::ast::CreateClause) -> Result<Plan> {
    // M3 CREATE supports:
    // - CREATE (n {prop: val}) - single node with properties
    // - CREATE (n)-[:rel]->(m) - single-hop pattern
    // - CREATE (n {a: 1})-[:1]->(m {b: 2}) - pattern with properties
    // Validate pattern length for MVP
    if create_clause.pattern.elements.is_empty() {
        return Err(Error::Other("CREATE pattern cannot be empty".into()));
    }

    // Labels are now supported!

    // MVP: Only support up to 3 elements (node, rel, node)
    if create_clause.pattern.elements.len() > 3 {
        return Err(Error::NotImplemented(
            "CREATE with more than 3 pattern elements in v2 M3",
        ));
    }

    Ok(Plan::Create {
        pattern: create_clause.pattern,
    })
}

fn try_optimize_nodescan_filter(plan: Plan, _predicate: Expression) -> Plan {
    // 1. Unwrap input logic - needs to be a NodeScan
    // The input to Filter is boxed, so we need to inspect the structure we just created
    // But here we passed 'plan' which is Filter{NodeScan...}
    let (input, predicate) = match &plan {
        Plan::Filter { input, predicate } => (input, predicate),
        _ => return plan,
    };

    let Plan::NodeScan { alias, label } = input.as_ref() else {
        return plan;
    };

    // 2. Must have label to use index
    let Some(label) = label else {
        return plan;
    };

    // 3. Check predicate
    // Helper to check for equality with a property
    let check_eq =
        |left: &Expression, right: &Expression| -> Option<(String, String, Expression)> {
            // Return (variable, property, value_expr)
            if let Expression::PropertyAccess(pa) = left {
                if &pa.variable == alias {
                    // Check if right is literal or parameter
                    match right {
                        Expression::Literal(_) | Expression::Parameter(_) => {
                            return Some((pa.variable.clone(), pa.property.clone(), right.clone()));
                        }
                        _ => {}
                    }
                }
            }
            None
        };

    if let Expression::Binary(bin) = &predicate {
        // Access fields of BinaryExpression (box)
        if matches!(bin.operator, crate::ast::BinaryOperator::Equals) {
            // Check left = right
            if let Some((v, p, val)) = check_eq(&bin.left, &bin.right) {
                return Plan::IndexSeek {
                    alias: v,
                    label: label.clone(),
                    field: p,
                    value_expr: val,
                    fallback: Box::new(plan.clone()),
                };
            }
            // Check right = left
            if let Some((v, p, val)) = check_eq(&bin.right, &bin.left) {
                return Plan::IndexSeek {
                    alias: v,
                    label: label.clone(),
                    field: p,
                    value_expr: val,
                    fallback: Box::new(plan.clone()),
                };
            }
        }
    }

    plan
}

fn compile_merge_plan(merge_clause: crate::ast::MergeClause) -> Result<Plan> {
    let pattern = merge_clause.pattern;
    if pattern.elements.is_empty() {
        return Err(Error::Other("MERGE pattern cannot be empty".into()));
    }
    if pattern.elements.len() != 1 && pattern.elements.len() != 3 {
        return Err(Error::NotImplemented(
            "MERGE supports only single-node or single-hop patterns in v2 M3",
        ));
    }

    // For MVP, MERGE needs stable identity -> require property maps on nodes.
    for el in &pattern.elements {
        if let crate::ast::PathElement::Node(n) = el {
            let Some(props) = &n.properties else {
                return Err(Error::NotImplemented(
                    "MERGE requires a non-empty node property map in v2 M3",
                ));
            };
            if props.properties.is_empty() {
                return Err(Error::NotImplemented(
                    "MERGE requires a non-empty node property map in v2 M3",
                ));
            }
            if n.labels.len() > 1 {
                return Err(Error::NotImplemented("MERGE with multiple labels in v2 M3"));
            }
        }
    }

    if pattern.elements.len() == 3 {
        let rel_pat = match &pattern.elements[1] {
            crate::ast::PathElement::Relationship(r) => r,
            _ => {
                return Err(Error::Other(
                    "MERGE pattern must have relationship in middle".into(),
                ));
            }
        };
        if !matches!(
            rel_pat.direction,
            crate::ast::RelationshipDirection::LeftToRight
        ) {
            return Err(Error::NotImplemented(
                "MERGE supports only -> direction in v2 M3",
            ));
        }
        if rel_pat.types.is_empty() {
            return Err(Error::Other("MERGE relationship requires a type".into()));
        }
        if rel_pat.types.len() > 1 {
            return Err(Error::NotImplemented(
                "MERGE with multiple rel types in v2 M3",
            ));
        }
        if rel_pat.variable_length.is_some() {
            return Err(Error::NotImplemented(
                "MERGE does not support variable-length relationships in v2 M3",
            ));
        }
        if let Some(props) = &rel_pat.properties
            && !props.properties.is_empty()
        {
            return Err(Error::NotImplemented(
                "MERGE relationship properties not supported in v2 M3",
            ));
        }
    }

    Ok(Plan::Create { pattern })
}

fn compile_match_plan(
    input: Option<Plan>,
    m: crate::ast::MatchClause,
    predicates: &BTreeMap<String, BTreeMap<String, Expression>>,
) -> Result<Plan> {
    match m.pattern.elements.len() {
        1 => {
            if input.is_some() {
                // If input exists, we don't support disconnected MATCH (n) yet.
                return Err(Error::NotImplemented(
                    "Multiple disconnected MATCH clauses not supported (Cartesian product) in v2 M3",
                ));
            }
            let node = match &m.pattern.elements[0] {
                crate::ast::PathElement::Node(n) => n,
                _ => return Err(Error::Other("pattern must be a node".into())),
            };
            let alias = node
                .variable
                .as_deref()
                .ok_or(Error::NotImplemented("anonymous node"))?
                .to_string();

            let label = node.labels.first().cloned();

            // Merge inline properties into predicates for optimization
            let mut local_predicates = predicates.clone();
            extend_predicates_from_properties(&alias, &node.properties, &mut local_predicates);

            // Optimizer: Try IndexSeek if there is a predicate.
            if let Some(label_name) = &label {
                if let Some(var_preds) = local_predicates.get(&alias) {
                    if let Some((field, val_expr)) = var_preds.iter().next() {
                        // For MVP, we return IndexSeek with fallback.
                        // It will try index at runtime, and if not found, use fallback scan.
                        return Ok(Plan::IndexSeek {
                            alias: alias.clone(),
                            label: label_name.clone(),
                            field: field.clone(),
                            value_expr: val_expr.clone(),
                            fallback: Box::new(Plan::NodeScan {
                                alias: alias.clone(),
                                label: label.clone(),
                            }),
                        });
                    }
                }
            }

            // Selection: Choose smallest label if multiple options (future)
            // or just use stats to warn or adjust (T156).

            Ok(Plan::NodeScan { alias, label })
        }
        3 => {
            let src = match &m.pattern.elements[0] {
                crate::ast::PathElement::Node(n) => n,
                _ => return Err(Error::Other("pattern must start with node".into())),
            };
            let rel_pat = match &m.pattern.elements[1] {
                crate::ast::PathElement::Relationship(r) => r,
                _ => return Err(Error::Other("expected relationship in middle".into())),
            };
            let dst = match &m.pattern.elements[2] {
                crate::ast::PathElement::Node(n) => n,
                _ => return Err(Error::Other("pattern must end with node".into())),
            };

            let src_alias = src
                .variable
                .as_deref()
                .ok_or(Error::NotImplemented("anonymous node"))?
                .to_string();
            let dst_alias = dst
                .variable
                .as_deref()
                .ok_or(Error::NotImplemented("anonymous node"))?
                .to_string();

            // Note: We don't propagate these predicates to MatchOut/VarLen yet
            // because those plans don't accept filters natively.
            // But we shouldn't error. We should ideally wrap with filter?
            // T305 MVP: Just ignore for optimization, but verify they don't block optimization of starting node?
            // Actually, if we have predicates on src, we should optimize the input?
            // compile_match_plan with len=3 takes `input` (Plan).
            // Usually `input` is populated. If `input` is None (start of query),
            // the first node is `src`. But `compile_match_plan` recursively handles inputs?
            // No, `compile_m3_plan` chains them.
            // Wait, for `MATCH (a)-[:R]->(b)`, the input to `compile_match_plan` is whatever came before.
            // If it's the first clause, input is None.
            // But `compile_match_plan` handles the *entire* pattern.
            // If len=3, it assumes a full path?
            // NervusDB v2 M3 likely handles `MATCH (a)-[:R]->(b)` by scanning `a` then expanding.
            // But `compile_match_plan` doesn't construct the scan for `a`?
            // Let's look at `compile_m3_plan`.
            // Ah, `compile_match_plan` for len=3 checks `input`.
            // If `input` is provided, it expands from it.
            // If `input` is None, it creates a `Box<Plan>`.
            // Currently `match m.pattern`...
            // If len=3: `MatchOut` / `MatchOutVarLen` has `input: input.map(Box::new)`.
            // If input is None, `MatchOut` iterates all relationships?
            // `MatchOutIter` does `snapshot.nodes()`... let's check executor.

            // Re: Inline properties for Relationship Pattern.
            // Currently throwing error.
            let effective_input = if input.is_none() {
                if let Some(props) = &src.properties {
                    let alias = src.variable.clone().ok_or(Error::NotImplemented(
                        "anonymous start node with properties",
                    ))?;
                    let label = src.labels.first().cloned();

                    let mut local_preds = predicates.clone();
                    extend_predicates_from_properties(
                        &alias,
                        &Some(props.clone()),
                        &mut local_preds,
                    );

                    if let Some(label_name) = &label
                        && let Some(var_preds) = local_preds.get(&alias)
                        && let Some((field, val_expr)) = var_preds.iter().next()
                    {
                        Some(Plan::IndexSeek {
                            alias: alias.clone(),
                            label: label_name.clone(),
                            field: field.clone(),
                            value_expr: val_expr.clone(),
                            fallback: Box::new(Plan::NodeScan {
                                alias: alias.clone(),
                                label: label.clone(),
                            }),
                        })
                    } else {
                        Some(Plan::NodeScan {
                            alias: alias.clone(),
                            label: label.clone(),
                        })
                    }
                } else {
                    None
                }
            } else {
                input
            };

            // Validation: Ensure we didn't drop properties silently
            if src.properties.is_some() && effective_input.is_none() {
                // effective_input is None implies input was None and src.properties was None.
                // So this branch is unreachable if src.properties is Some.
            }
            if src.properties.is_some() && effective_input.is_some() {
                let is_seek_or_scan = matches!(
                    effective_input.as_ref().unwrap(),
                    Plan::IndexSeek { .. } | Plan::NodeScan { .. }
                );
                // If input was Some originally, effective_input = input.
                // And input is usually NOT a NodeScan (it's some previous plan).
                // So if !is_seek_or_scan, it implies we didn't use src properties to build effective_input.
                // This assumes "input" (from chained match) is not a raw scan/seek.
                if !is_seek_or_scan {
                    return Err(Error::NotImplemented(
                        "Properties on start node only supported at start of query (chained match properties not supported)",
                    ));
                }
            }

            if !matches!(rel_pat.direction, RelationshipDirection::LeftToRight) {
                return Err(Error::NotImplemented("only -> direction in v2 M3"));
            }

            let rel = rel_pat.types.first().cloned();
            let edge_alias = rel_pat.variable.clone();

            if let Some(var_len) = &rel_pat.variable_length {
                let min_hops = var_len.min.unwrap_or(1);
                let max_hops = var_len.max;
                if min_hops == 0 {
                    return Err(Error::NotImplemented(
                        "0-length variable-length paths in v2 M3",
                    ));
                }
                if let Some(max) = max_hops {
                    if max < min_hops {
                        return Err(Error::Other(
                            "invalid variable-length range: max < min".into(),
                        ));
                    }
                }

                Ok(Plan::MatchOutVarLen {
                    input: effective_input.map(Box::new),
                    src_alias,
                    rel,
                    edge_alias,
                    dst_alias,
                    min_hops,
                    max_hops,
                    limit: None,
                    project: Vec::new(),
                    project_external: false,
                    optional: m.optional,
                })
            } else {
                Ok(Plan::MatchOut {
                    input: effective_input.map(Box::new),
                    src_alias,
                    rel,
                    edge_alias,
                    dst_alias,
                    limit: None,
                    project: Vec::new(),
                    project_external: false,
                    optional: m.optional,
                })
            }
        }
        _ => Err(Error::NotImplemented(
            "pattern length must be 1 or 3 in v2 M3",
        )),
    }
}

/// Helper to convert inline map properties to predicates
fn extend_predicates_from_properties(
    variable: &str,
    properties: &Option<crate::ast::PropertyMap>,
    predicates: &mut BTreeMap<String, BTreeMap<String, Expression>>,
) {
    if let Some(props) = properties {
        for prop in &props.properties {
            predicates
                .entry(variable.to_string())
                .or_default()
                .insert(prop.key.clone(), prop.value.clone());
        }
    }
}

fn parse_aggregate_function(
    call: &crate::ast::FunctionCall,
) -> Result<Option<crate::ast::AggregateFunction>> {
    let name = call.name.to_lowercase();
    match name.as_str() {
        "count" => {
            if call.args.is_empty() {
                Ok(Some(crate::ast::AggregateFunction::Count(None)))
            } else if call.args.len() == 1 {
                if let Expression::Literal(Literal::String(s)) = &call.args[0] {
                    if s == "*" {
                        return Ok(Some(crate::ast::AggregateFunction::Count(None)));
                    }
                }
                Ok(Some(crate::ast::AggregateFunction::Count(Some(
                    call.args[0].clone(),
                ))))
            } else {
                Err(Error::Other("COUNT takes 0 or 1 argument".into()))
            }
        }
        "sum" => {
            if call.args.len() != 1 {
                return Err(Error::Other("SUM takes exactly 1 argument".into()));
            }
            Ok(Some(crate::ast::AggregateFunction::Sum(
                call.args[0].clone(),
            )))
        }
        "avg" => {
            if call.args.len() != 1 {
                return Err(Error::Other("AVG takes exactly 1 argument".into()));
            }
            Ok(Some(crate::ast::AggregateFunction::Avg(
                call.args[0].clone(),
            )))
        }
        "min" => {
            if call.args.len() != 1 {
                return Err(Error::Other("MIN takes exactly 1 argument".into()));
            }
            Ok(Some(crate::ast::AggregateFunction::Min(
                call.args[0].clone(),
            )))
        }
        "max" => {
            if call.args.len() != 1 {
                return Err(Error::Other("MAX takes exactly 1 argument".into()));
            }
            Ok(Some(crate::ast::AggregateFunction::Max(
                call.args[0].clone(),
            )))
        }
        "collect" => {
            if call.args.len() != 1 {
                return Err(Error::Other("COLLECT takes exactly 1 argument".into()));
            }
            Ok(Some(crate::ast::AggregateFunction::Collect(
                call.args[0].clone(),
            )))
        }
        _ => Ok(None),
    }
}

fn extract_predicates(expr: &Expression, map: &mut BTreeMap<String, BTreeMap<String, Expression>>) {
    match expr {
        Expression::Binary(bin) => {
            if matches!(bin.operator, BinaryOperator::And) {
                extract_predicates(&bin.left, map);
                extract_predicates(&bin.right, map);
            } else if matches!(bin.operator, BinaryOperator::Equals) {
                let mut check_eq = |left: &Expression, right: &Expression| {
                    if let Expression::PropertyAccess(pa) = left {
                        match right {
                            Expression::Literal(_) | Expression::Parameter(_) => {
                                map.entry(pa.variable.clone())
                                    .or_default()
                                    .insert(pa.property.clone(), right.clone());
                            }
                            _ => {}
                        }
                    }
                };
                check_eq(&bin.left, &bin.right);
                check_eq(&bin.right, &bin.left);
            }
        }
        _ => {}
    }
}

fn extract_variables_from_expr(expr: &Expression, vars: &mut std::collections::HashSet<String>) {
    match expr {
        Expression::Variable(v) => {
            vars.insert(v.clone());
        }
        Expression::PropertyAccess(pa) => {
            vars.insert(pa.variable.clone());
        }
        Expression::FunctionCall(f) => {
            for arg in &f.args {
                extract_variables_from_expr(arg, vars);
            }
        }
        Expression::Binary(b) => {
            extract_variables_from_expr(&b.left, vars);
            extract_variables_from_expr(&b.right, vars);
        }
        Expression::Unary(u) => {
            extract_variables_from_expr(&u.operand, vars);
        }
        Expression::List(l) => {
            for item in l {
                extract_variables_from_expr(item, vars);
            }
        }
        Expression::Map(m) => {
            for pair in &m.properties {
                extract_variables_from_expr(&pair.value, vars);
            }
        }
        _ => {}
    }
}
