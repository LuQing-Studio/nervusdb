use super::{
    Error, PreparedQuery, Result, VecDeque, compile_m3_plan, render_plan, strip_explain_prefix,
};

pub(super) fn prepare(cypher: &str) -> Result<PreparedQuery> {
    if let Some(inner) = strip_explain_prefix(cypher) {
        if inner.is_empty() {
            return Err(Error::Other("EXPLAIN requires a query".into()));
        }
        let (query, merge_subclauses) = crate::parser::Parser::parse_with_merge_subclauses(inner)?;
        let mut merge_subclauses = VecDeque::from(merge_subclauses);
        let compiled = compile_m3_plan(query, &mut merge_subclauses, None)?;
        if !merge_subclauses.is_empty() {
            return Err(Error::Other(
                "internal error: unconsumed MERGE subclauses".into(),
            ));
        }
        let explain = Some(render_plan(&compiled.plan));
        return Ok(PreparedQuery {
            plan: compiled.plan,
            explain,
            write: compiled.write,
            merge_on_create_items: compiled.merge_on_create_items,
            merge_on_match_items: compiled.merge_on_match_items,
        });
    }

    let (query, merge_subclauses) = crate::parser::Parser::parse_with_merge_subclauses(cypher)?;
    let mut merge_subclauses = VecDeque::from(merge_subclauses);
    let compiled = compile_m3_plan(query, &mut merge_subclauses, None)?;
    if !merge_subclauses.is_empty() {
        return Err(Error::Other(
            "internal error: unconsumed MERGE subclauses".into(),
        ));
    }
    Ok(PreparedQuery {
        plan: compiled.plan,
        explain: None,
        write: compiled.write,
        merge_on_create_items: compiled.merge_on_create_items,
        merge_on_match_items: compiled.merge_on_match_items,
    })
}
