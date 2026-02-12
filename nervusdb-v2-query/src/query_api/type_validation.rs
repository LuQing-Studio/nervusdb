use super::{BinaryOperator, Clause, Error, Expression, Literal, Result};

fn is_definitely_non_boolean(expr: &Expression) -> bool {
    match expr {
        Expression::Literal(Literal::Boolean(_) | Literal::Null) => false,
        Expression::Literal(_) | Expression::List(_) | Expression::Map(_) => true,
        Expression::Unary(u) => match u.operator {
            crate::ast::UnaryOperator::Not => is_definitely_non_boolean(&u.operand),
            crate::ast::UnaryOperator::Negate => true,
        },
        Expression::Binary(b) => match b.operator {
            BinaryOperator::Equals
            | BinaryOperator::NotEquals
            | BinaryOperator::LessThan
            | BinaryOperator::LessEqual
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterEqual
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::Xor
            | BinaryOperator::In
            | BinaryOperator::StartsWith
            | BinaryOperator::EndsWith
            | BinaryOperator::Contains
            | BinaryOperator::HasLabel
            | BinaryOperator::IsNull
            | BinaryOperator::IsNotNull => false,
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Power => true,
        },
        Expression::Parameter(_)
        | Expression::Variable(_)
        | Expression::PropertyAccess(_)
        | Expression::FunctionCall(_)
        | Expression::Case(_)
        | Expression::Exists(_)
        | Expression::ListComprehension(_)
        | Expression::PatternComprehension(_) => false,
    }
}

fn is_definitely_non_list_literal(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Literal(
            Literal::Boolean(_) | Literal::Integer(_) | Literal::Float(_) | Literal::String(_)
        ) | Expression::Map(_)
    )
}

pub(super) fn validate_expression_types(expr: &Expression) -> Result<()> {
    match expr {
        Expression::Unary(u) => {
            validate_expression_types(&u.operand)?;
            if matches!(u.operator, crate::ast::UnaryOperator::Not)
                && is_definitely_non_boolean(&u.operand)
            {
                return Err(Error::Other(
                    "syntax error: InvalidArgumentType".to_string(),
                ));
            }
            Ok(())
        }
        Expression::Binary(b) => {
            validate_expression_types(&b.left)?;
            validate_expression_types(&b.right)?;
            if matches!(b.operator, BinaryOperator::In) && is_definitely_non_list_literal(&b.right)
            {
                return Err(Error::Other(
                    "syntax error: InvalidArgumentType".to_string(),
                ));
            }
            if matches!(
                b.operator,
                BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor
            ) && (is_definitely_non_boolean(&b.left) || is_definitely_non_boolean(&b.right))
            {
                return Err(Error::Other(
                    "syntax error: InvalidArgumentType".to_string(),
                ));
            }
            Ok(())
        }
        Expression::FunctionCall(call) => {
            for arg in &call.args {
                validate_expression_types(arg)?;
            }
            if call.name.eq_ignore_ascii_case("properties") {
                if call.args.len() != 1 {
                    return Err(Error::Other(
                        "syntax error: InvalidArgumentType".to_string(),
                    ));
                }
                if matches!(
                    call.args[0],
                    Expression::Literal(Literal::Integer(_) | Literal::Float(_))
                        | Expression::Literal(Literal::String(_))
                        | Expression::Literal(Literal::Boolean(_))
                        | Expression::List(_)
                ) {
                    return Err(Error::Other(
                        "syntax error: InvalidArgumentType".to_string(),
                    ));
                }
            }
            Ok(())
        }
        Expression::List(items) => {
            for item in items {
                validate_expression_types(item)?;
            }
            Ok(())
        }
        Expression::ListComprehension(list_comp) => {
            validate_expression_types(&list_comp.list)?;
            if let Some(where_expr) = &list_comp.where_expression {
                validate_expression_types(where_expr)?;
            }
            if let Some(map_expr) = &list_comp.map_expression {
                validate_expression_types(map_expr)?;
            }
            Ok(())
        }
        Expression::PatternComprehension(pattern_comp) => {
            for element in &pattern_comp.pattern.elements {
                match element {
                    crate::ast::PathElement::Node(node) => {
                        if let Some(props) = &node.properties {
                            for pair in &props.properties {
                                validate_expression_types(&pair.value)?;
                            }
                        }
                    }
                    crate::ast::PathElement::Relationship(rel) => {
                        if let Some(props) = &rel.properties {
                            for pair in &props.properties {
                                validate_expression_types(&pair.value)?;
                            }
                        }
                    }
                }
            }
            if let Some(where_expr) = &pattern_comp.where_expression {
                validate_expression_types(where_expr)?;
            }
            validate_expression_types(&pattern_comp.projection)?;
            Ok(())
        }
        Expression::Map(map) => {
            for pair in &map.properties {
                validate_expression_types(&pair.value)?;
            }
            Ok(())
        }
        Expression::Case(case_expr) => {
            if let Some(test) = &case_expr.expression {
                validate_expression_types(test)?;
            }
            for (when_expr, then_expr) in &case_expr.when_clauses {
                validate_expression_types(when_expr)?;
                validate_expression_types(then_expr)?;
            }
            if let Some(otherwise) = &case_expr.else_expression {
                validate_expression_types(otherwise)?;
            }
            Ok(())
        }
        Expression::Exists(exists) => {
            match exists.as_ref() {
                crate::ast::ExistsExpression::Pattern(pattern) => {
                    for element in &pattern.elements {
                        match element {
                            crate::ast::PathElement::Node(node) => {
                                if let Some(props) = &node.properties {
                                    for pair in &props.properties {
                                        validate_expression_types(&pair.value)?;
                                    }
                                }
                            }
                            crate::ast::PathElement::Relationship(rel) => {
                                if let Some(props) = &rel.properties {
                                    for pair in &props.properties {
                                        validate_expression_types(&pair.value)?;
                                    }
                                }
                            }
                        }
                    }
                }
                crate::ast::ExistsExpression::Subquery(subquery) => {
                    for clause in &subquery.clauses {
                        match clause {
                            Clause::Where(w) => validate_expression_types(&w.expression)?,
                            Clause::With(w) => {
                                for item in &w.items {
                                    validate_expression_types(&item.expression)?;
                                }
                                if let Some(where_clause) = &w.where_clause {
                                    validate_expression_types(&where_clause.expression)?;
                                }
                            }
                            Clause::Return(r) => {
                                for item in &r.items {
                                    validate_expression_types(&item.expression)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
