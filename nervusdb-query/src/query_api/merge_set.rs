use super::{BTreeSet, Error, Expression, Result};

pub(super) fn extract_merge_pattern_vars(pattern: &crate::ast::Pattern) -> BTreeSet<String> {
    let mut vars = BTreeSet::new();
    for el in &pattern.elements {
        match el {
            crate::ast::PathElement::Node(n) => {
                if let Some(v) = &n.variable {
                    vars.insert(v.clone());
                }
            }
            crate::ast::PathElement::Relationship(r) => {
                if let Some(v) = &r.variable {
                    vars.insert(v.clone());
                }
            }
        }
    }
    vars
}

pub(super) fn compile_merge_set_items(
    merge_vars: &BTreeSet<String>,
    set_clauses: Vec<crate::ast::SetClause>,
) -> Result<Vec<(String, String, Expression)>> {
    let mut items = Vec::new();
    for set_clause in set_clauses {
        for item in set_clause.items {
            if !merge_vars.contains(&item.property.variable) {
                return Err(Error::Other(format!(
                    "syntax error: UndefinedVariable ({})",
                    item.property.variable
                )));
            }
            items.push((item.property.variable, item.property.property, item.value));
        }
    }
    Ok(items)
}
