use super::{
    BTreeMap, BindingKind, Clause, Error, Expression, HashSet, Result,
    infer_expression_binding_kind,
};

fn is_quantifier_call(call: &crate::ast::FunctionCall) -> bool {
    matches!(
        call.name.as_str(),
        "__quant_any" | "__quant_all" | "__quant_none" | "__quant_single"
    )
}

pub(super) fn validate_where_expression_bindings(
    expr: &Expression,
    known_bindings: &BTreeMap<String, BindingKind>,
) -> Result<()> {
    let mut local_scopes: Vec<HashSet<String>> = Vec::new();
    validate_where_expression_variables(expr, known_bindings, &mut local_scopes)?;

    validate_pattern_predicate_bindings(expr, known_bindings)?;
    if matches!(
        infer_expression_binding_kind(expr, known_bindings),
        BindingKind::Node | BindingKind::Relationship | BindingKind::Path
    ) {
        return Err(Error::Other(
            "syntax error: InvalidArgumentType".to_string(),
        ));
    }
    Ok(())
}

fn validate_where_expression_variables(
    expr: &Expression,
    known_bindings: &BTreeMap<String, BindingKind>,
    local_scopes: &mut Vec<HashSet<String>>,
) -> Result<()> {
    match expr {
        Expression::Variable(var) => {
            if !is_locally_bound(local_scopes, var) && !known_bindings.contains_key(var) {
                return Err(Error::Other(format!(
                    "syntax error: UndefinedVariable ({})",
                    var
                )));
            }
        }
        Expression::PropertyAccess(pa) => {
            if !is_locally_bound(local_scopes, &pa.variable)
                && !known_bindings.contains_key(&pa.variable)
            {
                return Err(Error::Other(format!(
                    "syntax error: UndefinedVariable ({})",
                    pa.variable
                )));
            }
        }
        Expression::Unary(u) => {
            validate_where_expression_variables(&u.operand, known_bindings, local_scopes)?;
        }
        Expression::Binary(b) => {
            validate_where_expression_variables(&b.left, known_bindings, local_scopes)?;
            validate_where_expression_variables(&b.right, known_bindings, local_scopes)?;
        }
        Expression::FunctionCall(call) => {
            if is_quantifier_call(call) && call.args.len() == 3 {
                validate_where_expression_variables(&call.args[1], known_bindings, local_scopes)?;
                if let Expression::Variable(var) = &call.args[0] {
                    let mut scope = HashSet::new();
                    scope.insert(var.clone());
                    local_scopes.push(scope);
                    validate_where_expression_variables(
                        &call.args[2],
                        known_bindings,
                        local_scopes,
                    )?;
                    local_scopes.pop();
                } else {
                    validate_where_expression_variables(
                        &call.args[2],
                        known_bindings,
                        local_scopes,
                    )?;
                }
            } else {
                for arg in &call.args {
                    validate_where_expression_variables(arg, known_bindings, local_scopes)?;
                }
            }
        }
        Expression::List(items) => {
            for item in items {
                validate_where_expression_variables(item, known_bindings, local_scopes)?;
            }
        }
        Expression::Map(map) => {
            for pair in &map.properties {
                validate_where_expression_variables(&pair.value, known_bindings, local_scopes)?;
            }
        }
        Expression::Case(case_expr) => {
            if let Some(test_expr) = &case_expr.expression {
                validate_where_expression_variables(test_expr, known_bindings, local_scopes)?;
            }
            for (when_expr, then_expr) in &case_expr.when_clauses {
                validate_where_expression_variables(when_expr, known_bindings, local_scopes)?;
                validate_where_expression_variables(then_expr, known_bindings, local_scopes)?;
            }
            if let Some(else_expr) = &case_expr.else_expression {
                validate_where_expression_variables(else_expr, known_bindings, local_scopes)?;
            }
        }
        Expression::ListComprehension(list_comp) => {
            validate_where_expression_variables(&list_comp.list, known_bindings, local_scopes)?;
            let mut scope = HashSet::new();
            scope.insert(list_comp.variable.clone());
            local_scopes.push(scope);
            if let Some(where_expr) = &list_comp.where_expression {
                validate_where_expression_variables(where_expr, known_bindings, local_scopes)?;
            }
            if let Some(map_expr) = &list_comp.map_expression {
                validate_where_expression_variables(map_expr, known_bindings, local_scopes)?;
            }
            local_scopes.pop();
        }
        Expression::PatternComprehension(pattern_comp) => {
            let scope = collect_pattern_local_variables(&pattern_comp.pattern);
            local_scopes.push(scope);

            for element in &pattern_comp.pattern.elements {
                match element {
                    crate::ast::PathElement::Node(node) => {
                        if let Some(props) = &node.properties {
                            for pair in &props.properties {
                                validate_where_expression_variables(
                                    &pair.value,
                                    known_bindings,
                                    local_scopes,
                                )?;
                            }
                        }
                    }
                    crate::ast::PathElement::Relationship(rel) => {
                        if let Some(props) = &rel.properties {
                            for pair in &props.properties {
                                validate_where_expression_variables(
                                    &pair.value,
                                    known_bindings,
                                    local_scopes,
                                )?;
                            }
                        }
                    }
                }
            }

            if let Some(where_expr) = &pattern_comp.where_expression {
                validate_where_expression_variables(where_expr, known_bindings, local_scopes)?;
            }
            validate_where_expression_variables(
                &pattern_comp.projection,
                known_bindings,
                local_scopes,
            )?;

            local_scopes.pop();
        }
        Expression::Exists(_) | Expression::Parameter(_) | Expression::Literal(_) => {}
    }
    Ok(())
}

fn is_locally_bound(local_scopes: &[HashSet<String>], var: &str) -> bool {
    local_scopes.iter().rev().any(|scope| scope.contains(var))
}

fn collect_pattern_local_variables(pattern: &crate::ast::Pattern) -> HashSet<String> {
    let mut vars = HashSet::new();
    if let Some(path_var) = &pattern.variable {
        vars.insert(path_var.clone());
    }

    for element in &pattern.elements {
        match element {
            crate::ast::PathElement::Node(node) => {
                if let Some(var) = &node.variable {
                    vars.insert(var.clone());
                }
            }
            crate::ast::PathElement::Relationship(rel) => {
                if let Some(var) = &rel.variable {
                    vars.insert(var.clone());
                }
            }
        }
    }

    vars
}

fn validate_pattern_predicate_bindings(
    expr: &Expression,
    known_bindings: &BTreeMap<String, BindingKind>,
) -> Result<()> {
    match expr {
        Expression::Exists(exists_expr) => match exists_expr.as_ref() {
            crate::ast::ExistsExpression::Pattern(pattern) => {
                if pattern.elements.len() < 3 {
                    return Err(Error::Other(
                        "syntax error: InvalidArgumentType".to_string(),
                    ));
                }
                if let Some(path_var) = &pattern.variable
                    && !known_bindings.contains_key(path_var)
                {
                    return Err(Error::Other(format!(
                        "syntax error: UndefinedVariable ({})",
                        path_var
                    )));
                }
                for element in &pattern.elements {
                    match element {
                        crate::ast::PathElement::Node(node) => {
                            if let Some(var) = &node.variable
                                && !known_bindings.contains_key(var)
                            {
                                return Err(Error::Other(format!(
                                    "syntax error: UndefinedVariable ({})",
                                    var
                                )));
                            }
                            if let Some(props) = &node.properties {
                                for pair in &props.properties {
                                    validate_pattern_predicate_bindings(
                                        &pair.value,
                                        known_bindings,
                                    )?;
                                }
                            }
                        }
                        crate::ast::PathElement::Relationship(rel) => {
                            if let Some(var) = &rel.variable
                                && !known_bindings.contains_key(var)
                            {
                                return Err(Error::Other(format!(
                                    "syntax error: UndefinedVariable ({})",
                                    var
                                )));
                            }
                            if let Some(props) = &rel.properties {
                                for pair in &props.properties {
                                    validate_pattern_predicate_bindings(
                                        &pair.value,
                                        known_bindings,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            crate::ast::ExistsExpression::Subquery(subquery) => {
                for clause in &subquery.clauses {
                    match clause {
                        Clause::Where(w) => {
                            validate_pattern_predicate_bindings(&w.expression, known_bindings)?
                        }
                        Clause::With(w) => {
                            for item in &w.items {
                                validate_pattern_predicate_bindings(
                                    &item.expression,
                                    known_bindings,
                                )?;
                            }
                            if let Some(where_clause) = &w.where_clause {
                                validate_pattern_predicate_bindings(
                                    &where_clause.expression,
                                    known_bindings,
                                )?;
                            }
                        }
                        Clause::Return(r) => {
                            for item in &r.items {
                                validate_pattern_predicate_bindings(
                                    &item.expression,
                                    known_bindings,
                                )?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        },
        Expression::Unary(u) => validate_pattern_predicate_bindings(&u.operand, known_bindings)?,
        Expression::Binary(b) => {
            validate_pattern_predicate_bindings(&b.left, known_bindings)?;
            validate_pattern_predicate_bindings(&b.right, known_bindings)?;
        }
        Expression::FunctionCall(call) => {
            if is_quantifier_call(call) && call.args.len() == 3 {
                validate_pattern_predicate_bindings(&call.args[1], known_bindings)?;
                if let Expression::Variable(var) = &call.args[0] {
                    let mut scoped = known_bindings.clone();
                    scoped.entry(var.clone()).or_insert(BindingKind::Unknown);
                    validate_pattern_predicate_bindings(&call.args[2], &scoped)?;
                } else {
                    validate_pattern_predicate_bindings(&call.args[2], known_bindings)?;
                }
            } else {
                for arg in &call.args {
                    validate_pattern_predicate_bindings(arg, known_bindings)?;
                }
            }
        }
        Expression::List(items) => {
            for item in items {
                validate_pattern_predicate_bindings(item, known_bindings)?;
            }
        }
        Expression::ListComprehension(list_comp) => {
            validate_pattern_predicate_bindings(&list_comp.list, known_bindings)?;
            if let Some(where_expr) = &list_comp.where_expression {
                validate_pattern_predicate_bindings(where_expr, known_bindings)?;
            }
            if let Some(map_expr) = &list_comp.map_expression {
                validate_pattern_predicate_bindings(map_expr, known_bindings)?;
            }
        }
        Expression::PatternComprehension(pattern_comp) => {
            let mut scoped = known_bindings.clone();
            for var in collect_pattern_local_variables(&pattern_comp.pattern) {
                scoped.entry(var).or_insert(BindingKind::Unknown);
            }

            for element in &pattern_comp.pattern.elements {
                match element {
                    crate::ast::PathElement::Node(node) => {
                        if let Some(props) = &node.properties {
                            for pair in &props.properties {
                                validate_pattern_predicate_bindings(&pair.value, &scoped)?;
                            }
                        }
                    }
                    crate::ast::PathElement::Relationship(rel) => {
                        if let Some(props) = &rel.properties {
                            for pair in &props.properties {
                                validate_pattern_predicate_bindings(&pair.value, &scoped)?;
                            }
                        }
                    }
                }
            }

            if let Some(where_expr) = &pattern_comp.where_expression {
                validate_pattern_predicate_bindings(where_expr, &scoped)?;
            }
            validate_pattern_predicate_bindings(&pattern_comp.projection, &scoped)?;
        }
        Expression::Map(map) => {
            for pair in &map.properties {
                validate_pattern_predicate_bindings(&pair.value, known_bindings)?;
            }
        }
        Expression::Case(case_expr) => {
            if let Some(test_expr) = &case_expr.expression {
                validate_pattern_predicate_bindings(test_expr, known_bindings)?;
            }
            for (when_expr, then_expr) in &case_expr.when_clauses {
                validate_pattern_predicate_bindings(when_expr, known_bindings)?;
                validate_pattern_predicate_bindings(then_expr, known_bindings)?;
            }
            if let Some(else_expr) = &case_expr.else_expression {
                validate_pattern_predicate_bindings(else_expr, known_bindings)?;
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_where_expression_bindings;
    use crate::ast::{BinaryExpression, BinaryOperator, Expression, FunctionCall, Literal};
    use std::collections::BTreeMap;

    #[test]
    fn quantifier_variable_is_scoped_inside_where_validation() {
        let expr = Expression::FunctionCall(FunctionCall {
            name: "__quant_any".to_string(),
            args: vec![
                Expression::Variable("x".to_string()),
                Expression::Variable("list".to_string()),
                Expression::Binary(Box::new(BinaryExpression {
                    left: Expression::Variable("x".to_string()),
                    operator: BinaryOperator::Equals,
                    right: Expression::Literal(Literal::Integer(2)),
                })),
            ],
        });
        let mut known = BTreeMap::new();
        known.insert("list".to_string(), super::BindingKind::Unknown);
        validate_where_expression_bindings(&expr, &known)
            .expect("quantifier variable should be treated as local scope");
    }

    #[test]
    fn quantifier_predicate_still_rejects_unknown_outer_variable() {
        let expr = Expression::FunctionCall(FunctionCall {
            name: "__quant_any".to_string(),
            args: vec![
                Expression::Variable("x".to_string()),
                Expression::Variable("list".to_string()),
                Expression::Binary(Box::new(BinaryExpression {
                    left: Expression::Variable("y".to_string()),
                    operator: BinaryOperator::Equals,
                    right: Expression::Literal(Literal::Integer(2)),
                })),
            ],
        });
        let mut known = BTreeMap::new();
        known.insert("list".to_string(), super::BindingKind::Unknown);
        let err = validate_where_expression_bindings(&expr, &known)
            .expect_err("unknown variable should still be rejected");
        assert_eq!(err.to_string(), "syntax error: UndefinedVariable (y)");
    }
}
