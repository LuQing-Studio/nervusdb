use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use nervusdb::{
    NDB_ERRCAT_EXECUTION, NDB_OK, ndb_begin_write, ndb_close, ndb_db_t, ndb_execute_write,
    ndb_last_error_category, ndb_last_error_message, ndb_open, ndb_query, ndb_result_free,
    ndb_result_t, ndb_result_to_json, ndb_string_free, ndb_txn_commit, ndb_txn_query, ndb_txn_t,
};

#[test]
fn capi_smoke_query_write_and_txn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("capi-smoke");
    let db_path = CString::new(db_path.to_string_lossy().to_string()).expect("db path cstr");

    let mut db: *mut ndb_db_t = ptr::null_mut();
    assert_eq!(ndb_open(db_path.as_ptr(), &mut db), NDB_OK);
    assert!(!db.is_null());

    let create_sql = CString::new("CREATE (:User {name: 'alice'})").expect("create cstr");
    let mut write_count: u32 = 0;
    assert_eq!(
        ndb_execute_write(db, create_sql.as_ptr(), ptr::null(), &mut write_count),
        NDB_OK
    );
    assert!(write_count >= 1);

    let mut result: *mut ndb_result_t = ptr::null_mut();
    let query_sql = CString::new("MATCH (n:User) RETURN count(n) AS c").expect("query cstr");
    assert_eq!(
        ndb_query(db, query_sql.as_ptr(), ptr::null(), &mut result),
        NDB_OK
    );
    assert!(!result.is_null());

    let mut json_ptr: *mut c_char = ptr::null_mut();
    assert_eq!(ndb_result_to_json(result, &mut json_ptr), NDB_OK);
    assert!(!json_ptr.is_null());
    let json = unsafe { CStr::from_ptr(json_ptr) }
        .to_str()
        .expect("json utf8")
        .to_string();
    assert!(json.contains("\"c\":1") || json.contains("\"c\":1.0"));
    ndb_string_free(json_ptr);
    ndb_result_free(result);

    let mut txn: *mut ndb_txn_t = ptr::null_mut();
    assert_eq!(ndb_begin_write(db, &mut txn), NDB_OK);
    assert!(!txn.is_null());
    let tx_sql = CString::new("CREATE (:User {name: 'bob'})").expect("tx sql");
    assert_eq!(ndb_txn_query(txn, tx_sql.as_ptr(), ptr::null()), NDB_OK);
    assert_eq!(ndb_txn_commit(txn), NDB_OK);

    result = ptr::null_mut();
    assert_eq!(
        ndb_query(db, query_sql.as_ptr(), ptr::null(), &mut result),
        NDB_OK
    );
    json_ptr = ptr::null_mut();
    assert_eq!(ndb_result_to_json(result, &mut json_ptr), NDB_OK);
    let json2 = unsafe { CStr::from_ptr(json_ptr) }
        .to_str()
        .expect("json2 utf8")
        .to_string();
    assert!(json2.contains("\"c\":2") || json2.contains("\"c\":2.0"));
    ndb_string_free(json_ptr);
    ndb_result_free(result);

    assert_eq!(ndb_close(db), NDB_OK);
}

#[test]
fn capi_query_api_rejects_write_statement() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("capi-read-only");
    let db_path = CString::new(db_path.to_string_lossy().to_string()).expect("db path cstr");

    let mut db: *mut ndb_db_t = ptr::null_mut();
    assert_eq!(ndb_open(db_path.as_ptr(), &mut db), NDB_OK);

    let mut result: *mut ndb_result_t = ptr::null_mut();
    let write_sql = CString::new("CREATE (:Blocked)").expect("write sql");
    let rc = ndb_query(db, write_sql.as_ptr(), ptr::null(), &mut result);
    assert_ne!(rc, NDB_OK);
    assert_eq!(ndb_last_error_category(), NDB_ERRCAT_EXECUTION);

    let mut buf = vec![0 as c_char; 256];
    let copied = ndb_last_error_message(buf.as_mut_ptr(), buf.len());
    assert!(copied > 0);
    let msg = unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .expect("error message utf8");
    assert!(msg.contains("read"));

    assert_eq!(ndb_close(db), NDB_OK);
}
