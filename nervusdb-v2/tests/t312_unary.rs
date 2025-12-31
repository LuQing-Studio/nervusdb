// T312: Expression Precedence + Unary Operators - TDD Test Suite
// 🔴 RED Phase: All tests should FAIL initially
//
// Operators to implement/verify:
// - NOT (boolean negation)
// - Unary minus (-x)
// - Precedence: NOT > comparison > AND > OR

use nervusdb_v2::Db;
use nervusdb_v2::query::Value;
use tempfile::tempdir;

// ============================================================================
// NOT operator tests
// ============================================================================

#[test]
fn test_not_true() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    let query = "RETURN NOT true AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Bool(false));

    Ok(())
}

#[test]
fn test_not_false() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    let query = "RETURN NOT false AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Bool(true));

    Ok(())
}

#[test]
fn test_not_comparison() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    // NOT (1 = 2) should be true
    let query = "RETURN NOT (1 = 2) AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Bool(true));

    Ok(())
}

// ============================================================================
// Unary minus tests
// ============================================================================

#[test]
fn test_unary_minus() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    let query = "RETURN -5 AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Int(-5));

    Ok(())
}

#[test]
fn test_unary_minus_expression() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    // -(3 + 2) should be -5
    let query = "RETURN -(3 + 2) AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Int(-5));

    Ok(())
}

#[test]
fn test_double_negative() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    // --5 should be 5
    let query = "RETURN --5 AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Int(5));

    Ok(())
}

// ============================================================================
// Precedence tests
// ============================================================================

#[test]
fn test_not_and_precedence() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    // NOT true AND false should be (NOT true) AND false = false AND false = false
    let query = "RETURN NOT true AND false AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Bool(false));

    Ok(())
}

#[test]
fn test_and_or_precedence() -> nervusdb_v2::Result<()> {
    let dir = tempdir()?;
    let db_path = dir.path().join("t312.ndb");
    let db = Db::open(&db_path)?;

    // true OR false AND false should be true OR (false AND false) = true OR false = true
    let query = "RETURN true OR false AND false AS result";
    let prep = nervusdb_v2::query::prepare(query)?;
    let snapshot = db.snapshot();
    let results: Vec<_> = prep
        .execute_streaming(&snapshot, &Default::default())
        .collect::<Result<Vec<_>, _>>()?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("result").unwrap(), &Value::Bool(true));

    Ok(())
}
