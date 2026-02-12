use super::{BTreeMap, BindingKind};

pub(super) fn first_relationship_is_bound(
    pattern: &crate::ast::Pattern,
    known_bindings: &BTreeMap<String, BindingKind>,
) -> bool {
    match pattern.elements.get(1) {
        Some(crate::ast::PathElement::Relationship(rel)) => {
            rel.variable_length.is_none()
                && rel
                    .variable
                    .as_ref()
                    .and_then(|name| known_bindings.get(name))
                    .is_some_and(|kind| {
                        matches!(kind, BindingKind::Relationship | BindingKind::Unknown)
                    })
        }
        _ => false,
    }
}

pub(super) fn build_optional_unbind_aliases(
    known_bindings: &BTreeMap<String, BindingKind>,
    src_alias: &str,
    dst_alias: &str,
    edge_alias: Option<&str>,
    path_alias: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut push_alias = |alias: &str| {
        if !out.iter().any(|existing| existing == alias) {
            out.push(alias.to_string());
        }
    };

    if !is_binding_compatible(known_bindings, src_alias, BindingKind::Node) {
        push_alias(src_alias);
    }
    if !is_binding_compatible(known_bindings, dst_alias, BindingKind::Node) {
        push_alias(dst_alias);
    }
    if let Some(alias) = edge_alias
        && !is_binding_compatible(known_bindings, alias, BindingKind::Relationship)
    {
        push_alias(alias);
    }
    if let Some(alias) = path_alias
        && !is_binding_compatible(known_bindings, alias, BindingKind::Path)
    {
        push_alias(alias);
    }

    out
}

pub(super) fn maybe_reanchor_pattern(
    pattern: crate::ast::Pattern,
    known_bindings: &BTreeMap<String, BindingKind>,
) -> crate::ast::Pattern {
    if pattern.elements.len() != 3 {
        return pattern;
    }

    let (first, rel, last) = match (
        &pattern.elements[0],
        &pattern.elements[1],
        &pattern.elements[2],
    ) {
        (
            crate::ast::PathElement::Node(first),
            crate::ast::PathElement::Relationship(rel),
            crate::ast::PathElement::Node(last),
        ) => (first, rel, last),
        _ => return pattern,
    };

    let first_bound = is_bound_node_alias(first, known_bindings);
    let last_bound = is_bound_node_alias(last, known_bindings);

    if first_bound || !last_bound {
        return pattern;
    }

    let mut flipped_rel = rel.clone();
    flipped_rel.direction = reverse_relationship_direction(&flipped_rel.direction);

    crate::ast::Pattern {
        variable: pattern.variable,
        elements: vec![
            crate::ast::PathElement::Node(last.clone()),
            crate::ast::PathElement::Relationship(flipped_rel),
            crate::ast::PathElement::Node(first.clone()),
        ],
    }
}

fn reverse_relationship_direction(
    direction: &crate::ast::RelationshipDirection,
) -> crate::ast::RelationshipDirection {
    match direction {
        crate::ast::RelationshipDirection::LeftToRight => {
            crate::ast::RelationshipDirection::RightToLeft
        }
        crate::ast::RelationshipDirection::RightToLeft => {
            crate::ast::RelationshipDirection::LeftToRight
        }
        crate::ast::RelationshipDirection::Undirected => {
            crate::ast::RelationshipDirection::Undirected
        }
    }
}

fn is_bound_node_alias(
    node: &crate::ast::NodePattern,
    known_bindings: &BTreeMap<String, BindingKind>,
) -> bool {
    node.variable
        .as_ref()
        .and_then(|name| known_bindings.get(name))
        .is_some_and(|kind| matches!(kind, BindingKind::Node | BindingKind::Unknown))
}

fn is_binding_compatible(
    known_bindings: &BTreeMap<String, BindingKind>,
    alias: &str,
    expected: BindingKind,
) -> bool {
    matches!(
        known_bindings.get(alias),
        Some(kind) if *kind == expected || *kind == BindingKind::Unknown
    )
}
