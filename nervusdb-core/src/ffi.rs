//! C FFI bindings for NervusDB.
//!
//! Safety requirements are documented in the C header file: `include/nervusdb.h`
#![allow(clippy::missing_safety_doc)]

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;

use crate::{Database, Error, Options, QueryCriteria, Triple};

#[allow(non_camel_case_types)]
pub type nervusdb_status = i32;

pub const NERVUSDB_OK: nervusdb_status = 0;
pub const NERVUSDB_ERR_INVALID_ARGUMENT: nervusdb_status = 1;
pub const NERVUSDB_ERR_OPEN: nervusdb_status = 2;
pub const NERVUSDB_ERR_INTERNAL: nervusdb_status = 3;
pub const NERVUSDB_ERR_CALLBACK: nervusdb_status = 4;

#[repr(C)]
pub struct nervusdb_db {
    _private: [u8; 0],
}

#[repr(C)]
pub struct nervusdb_error {
    pub code: nervusdb_status,
    pub message: *mut c_char,
}

#[repr(C)]
pub struct nervusdb_query_criteria {
    pub subject_id: u64,
    pub predicate_id: u64,
    pub object_id: u64,
    pub has_subject: bool,
    pub has_predicate: bool,
    pub has_object: bool,
}

#[allow(non_camel_case_types)]
pub type nervusdb_triple_callback = Option<extern "C" fn(u64, u64, u64, *mut c_void) -> bool>;

static NERVUSDB_VERSION_STR: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

#[inline]
fn clear_error(out_error: *mut *mut nervusdb_error) {
    if out_error.is_null() {
        return;
    }
    unsafe {
        *out_error = ptr::null_mut();
    }
}

fn set_error(out_error: *mut *mut nervusdb_error, code: nervusdb_status, message: &str) {
    if out_error.is_null() {
        return;
    }

    let c_message =
        CString::new(message).unwrap_or_else(|_| CString::new("invalid error message").unwrap());
    let error = Box::new(nervusdb_error {
        code,
        message: c_message.into_raw(),
    });
    unsafe {
        *out_error = Box::into_raw(error);
    }
}

fn status_from_error(err: &Error) -> nervusdb_status {
    match err {
        Error::InvalidCursor(_) | Error::NotFound | Error::Other(_) => NERVUSDB_ERR_INTERNAL,
        _ => NERVUSDB_ERR_INTERNAL,
    }
}

fn db_from_ptr<'a>(
    db: *mut nervusdb_db,
    out_error: *mut *mut nervusdb_error,
) -> Result<&'a mut Database, nervusdb_status> {
    if db.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "database pointer is null",
        );
        return Err(NERVUSDB_ERR_INVALID_ARGUMENT);
    }
    unsafe { Ok(&mut *(db as *mut Database)) }
}

fn cstr_to_owned(
    value: *const c_char,
    out_error: *mut *mut nervusdb_error,
    name: &str,
) -> Result<String, nervusdb_status> {
    if value.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            &format!("{name} pointer is null"),
        );
        return Err(NERVUSDB_ERR_INVALID_ARGUMENT);
    }
    match unsafe { CStr::from_ptr(value) }.to_str() {
        Ok(v) => Ok(v.to_owned()),
        Err(_) => {
            set_error(
                out_error,
                NERVUSDB_ERR_INVALID_ARGUMENT,
                &format!("{name} is not valid UTF-8"),
            );
            Err(NERVUSDB_ERR_INVALID_ARGUMENT)
        }
    }
}

fn criteria_from_ffi(ffi: &nervusdb_query_criteria) -> QueryCriteria {
    QueryCriteria {
        subject_id: if ffi.has_subject {
            Some(ffi.subject_id)
        } else {
            None
        },
        predicate_id: if ffi.has_predicate {
            Some(ffi.predicate_id)
        } else {
            None
        },
        object_id: if ffi.has_object {
            Some(ffi.object_id)
        } else {
            None
        },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_version() -> *const c_char {
    NERVUSDB_VERSION_STR.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_free_string(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(value));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_open(
    path: *const c_char,
    out_db: *mut *mut nervusdb_db,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    if path.is_null() || out_db.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "path/out_db pointer is null",
        );
        return NERVUSDB_ERR_INVALID_ARGUMENT;
    }

    let path_str = match cstr_to_owned(path, out_error, "path") {
        Ok(v) => v,
        Err(code) => return code,
    };

    match Database::open(Options::new(path_str.as_str())) {
        Ok(db) => {
            let boxed: Box<Database> = Box::new(db);
            unsafe {
                *out_db = Box::into_raw(boxed) as *mut nervusdb_db;
            }
            NERVUSDB_OK
        }
        Err(err) => {
            set_error(out_error, NERVUSDB_ERR_OPEN, &err.to_string());
            NERVUSDB_ERR_OPEN
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_close(db: *mut nervusdb_db) {
    if db.is_null() {
        return;
    }
    let db_ptr = db as *mut Database;
    unsafe {
        drop(Box::from_raw(db_ptr));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_intern(
    db: *mut nervusdb_db,
    value: *const c_char,
    out_id: *mut u64,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    if value.is_null() || out_id.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "value/out_id pointer is null",
        );
        return NERVUSDB_ERR_INVALID_ARGUMENT;
    }

    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    let value_str = match cstr_to_owned(value, out_error, "value") {
        Ok(v) => v,
        Err(code) => return code,
    };

    match db.intern(value_str.as_str()) {
        Ok(id) => {
            unsafe {
                *out_id = id;
            }
            NERVUSDB_OK
        }
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_resolve_id(
    db: *mut nervusdb_db,
    value: *const c_char,
    out_id: *mut u64,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    if value.is_null() || out_id.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "value/out_id pointer is null",
        );
        return NERVUSDB_ERR_INVALID_ARGUMENT;
    }

    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    let value_str = match cstr_to_owned(value, out_error, "value") {
        Ok(v) => v,
        Err(code) => return code,
    };

    match db.resolve_id(value_str.as_str()) {
        Ok(Some(id)) => {
            unsafe {
                *out_id = id;
            }
            NERVUSDB_OK
        }
        Ok(None) => {
            unsafe {
                *out_id = 0;
            }
            NERVUSDB_OK
        }
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_resolve_str(
    db: *mut nervusdb_db,
    id: u64,
    out_value: *mut *mut c_char,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    if out_value.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "out_value pointer is null",
        );
        return NERVUSDB_ERR_INVALID_ARGUMENT;
    }

    unsafe {
        *out_value = ptr::null_mut();
    }

    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    match db.resolve_str(id) {
        Ok(Some(value)) => match CString::new(value) {
            Ok(c_value) => {
                unsafe {
                    *out_value = c_value.into_raw();
                }
                NERVUSDB_OK
            }
            Err(_) => {
                set_error(out_error, NERVUSDB_ERR_INTERNAL, "value contained NUL byte");
                NERVUSDB_ERR_INTERNAL
            }
        },
        Ok(None) => NERVUSDB_OK,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_add_triple(
    db: *mut nervusdb_db,
    subject_id: u64,
    predicate_id: u64,
    object_id: u64,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    let triple = Triple::new(subject_id, predicate_id, object_id);
    let insert_result = if let Some(txn) = db.active_write.as_mut() {
        crate::storage::disk::insert_triple(txn, &triple).map(|_| ())
    } else {
        db.store.insert(&triple).map(|_| ())
    };
    match insert_result {
        Ok(()) => NERVUSDB_OK,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_begin_transaction(
    db: *mut nervusdb_db,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };
    match db.begin_transaction() {
        Ok(()) => NERVUSDB_OK,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_commit_transaction(
    db: *mut nervusdb_db,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };
    match db.commit_transaction() {
        Ok(()) => NERVUSDB_OK,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_abort_transaction(
    db: *mut nervusdb_db,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };
    match db.abort_transaction() {
        Ok(()) => NERVUSDB_OK,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_query_triples(
    db: *mut nervusdb_db,
    criteria: *const nervusdb_query_criteria,
    callback: nervusdb_triple_callback,
    user_data: *mut c_void,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    let callback = match callback {
        Some(cb) => cb,
        None => {
            set_error(out_error, NERVUSDB_ERR_INVALID_ARGUMENT, "callback is null");
            return NERVUSDB_ERR_INVALID_ARGUMENT;
        }
    };

    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    let query = if criteria.is_null() {
        QueryCriteria::default()
    } else {
        unsafe { criteria_from_ffi(&*criteria) }
    };

    for triple in db.query(query) {
        let should_continue = callback(
            triple.subject_id,
            triple.predicate_id,
            triple.object_id,
            user_data,
        );
        if !should_continue {
            break;
        }
    }

    NERVUSDB_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_exec_cypher(
    db: *mut nervusdb_db,
    query: *const c_char,
    params_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut nervusdb_error,
) -> nervusdb_status {
    clear_error(out_error);
    if query.is_null() || out_json.is_null() {
        set_error(
            out_error,
            NERVUSDB_ERR_INVALID_ARGUMENT,
            "query/out_json pointer is null",
        );
        return NERVUSDB_ERR_INVALID_ARGUMENT;
    }

    unsafe {
        *out_json = ptr::null_mut();
    }

    let db = match db_from_ptr(db, out_error) {
        Ok(db) => db,
        Err(code) => return code,
    };

    let query_str = match cstr_to_owned(query, out_error, "query") {
        Ok(v) => v,
        Err(code) => return code,
    };

    let params: Option<HashMap<String, serde_json::Value>> = if params_json.is_null() {
        None
    } else {
        let raw = match cstr_to_owned(params_json, out_error, "params_json") {
            Ok(v) => v,
            Err(code) => return code,
        };
        if raw.trim().is_empty() {
            None
        } else {
            match serde_json::from_str::<HashMap<String, serde_json::Value>>(&raw) {
                Ok(map) => Some(map),
                Err(_) => {
                    set_error(
                        out_error,
                        NERVUSDB_ERR_INVALID_ARGUMENT,
                        "params_json must be a JSON object",
                    );
                    return NERVUSDB_ERR_INVALID_ARGUMENT;
                }
            }
        }
    };

    let results = match db.execute_query_with_params(query_str.as_str(), params) {
        Ok(r) => r,
        Err(err) => {
            let message = err.to_string();
            let code = status_from_error(&err);
            set_error(out_error, code, &message);
            return code;
        }
    };

    let json_results: Vec<HashMap<String, serde_json::Value>> = results
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|(k, v)| {
                    let json_val = match v {
                        crate::query::executor::Value::String(s) => serde_json::Value::String(s),
                        crate::query::executor::Value::Float(f) => serde_json::json!(f),
                        crate::query::executor::Value::Boolean(b) => serde_json::Value::Bool(b),
                        crate::query::executor::Value::Null => serde_json::Value::Null,
                        crate::query::executor::Value::Node(id) => serde_json::json!({ "id": id }),
                        crate::query::executor::Value::Relationship(id) => {
                            serde_json::json!({ "id": id })
                        }
                    };
                    (k, json_val)
                })
                .collect()
        })
        .collect();

    let json_string = match serde_json::to_string(&json_results) {
        Ok(s) => s,
        Err(_) => {
            set_error(
                out_error,
                NERVUSDB_ERR_INTERNAL,
                "failed to serialize results to JSON",
            );
            return NERVUSDB_ERR_INTERNAL;
        }
    };

    let c_json = match CString::new(json_string) {
        Ok(s) => s,
        Err(_) => {
            set_error(out_error, NERVUSDB_ERR_INTERNAL, "JSON contained NUL byte");
            return NERVUSDB_ERR_INTERNAL;
        }
    };

    unsafe {
        *out_json = c_json.into_raw();
    }

    NERVUSDB_OK
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nervusdb_free_error(err: *mut nervusdb_error) {
    if err.is_null() {
        return;
    }
    let boxed = unsafe { Box::from_raw(err) };
    if !boxed.message.is_null() {
        unsafe {
            drop(CString::from_raw(boxed.message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    static CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn collect(_s: u64, _p: u64, _o: u64, _data: *mut c_void) -> bool {
        CALLBACK_COUNT.fetch_add(1, Ordering::SeqCst);
        true
    }

    #[test]
    fn ffi_roundtrip() {
        unsafe {
            CALLBACK_COUNT.store(0, Ordering::SeqCst);
            let dir = tempdir().unwrap();
            let path = dir.path().join("ffi_roundtrip");
            let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();

            let mut db_ptr: *mut nervusdb_db = ptr::null_mut();
            let mut err_ptr: *mut nervusdb_error = ptr::null_mut();
            let status = nervusdb_open(path_c.as_ptr(), &mut db_ptr, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(!db_ptr.is_null());
            assert!(err_ptr.is_null());

            let mut id = 0u64;
            let name = CString::new("Alice").unwrap();
            let status = nervusdb_intern(db_ptr, name.as_ptr(), &mut id, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(id > 0);
            assert!(err_ptr.is_null());

            let mut resolved_id = 0u64;
            let status = nervusdb_resolve_id(db_ptr, name.as_ptr(), &mut resolved_id, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());
            assert_eq!(resolved_id, id);

            let mut resolved_str: *mut c_char = ptr::null_mut();
            let status = nervusdb_resolve_str(db_ptr, id, &mut resolved_str, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());
            assert!(!resolved_str.is_null());
            let roundtrip = CStr::from_ptr(resolved_str).to_string_lossy().to_string();
            assert_eq!(roundtrip, "Alice");
            nervusdb_free_string(resolved_str);

            let status = nervusdb_add_triple(db_ptr, id, id, id, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());

            let query_status = nervusdb_query_triples(
                db_ptr,
                ptr::null(),
                Some(collect),
                ptr::null_mut(),
                &mut err_ptr,
            );
            assert_eq!(query_status, NERVUSDB_OK);
            assert!(CALLBACK_COUNT.load(Ordering::SeqCst) >= 1);
            assert!(err_ptr.is_null());

            // Transaction API + exec_cypher smoke
            let status = nervusdb_begin_transaction(db_ptr, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());

            let alice = intern(db_ptr, "alice", &mut err_ptr);
            let name_id = intern(db_ptr, "name", &mut err_ptr);
            let alice_val = intern(db_ptr, "Alice", &mut err_ptr);
            let age_id = intern(db_ptr, "age", &mut err_ptr);
            let age_val = intern(db_ptr, "30", &mut err_ptr);
            let bob = intern(db_ptr, "bob", &mut err_ptr);
            let bob_val = intern(db_ptr, "Bob", &mut err_ptr);
            let knows_id = intern(db_ptr, "knows", &mut err_ptr);

            assert!(err_ptr.is_null());

            assert_eq!(
                nervusdb_add_triple(db_ptr, alice, name_id, alice_val, &mut err_ptr),
                NERVUSDB_OK
            );
            assert_eq!(
                nervusdb_add_triple(db_ptr, alice, age_id, age_val, &mut err_ptr),
                NERVUSDB_OK
            );
            assert_eq!(
                nervusdb_add_triple(db_ptr, bob, name_id, bob_val, &mut err_ptr),
                NERVUSDB_OK
            );
            assert_eq!(
                nervusdb_add_triple(db_ptr, alice, knows_id, bob, &mut err_ptr),
                NERVUSDB_OK
            );
            assert!(err_ptr.is_null());

            let status = nervusdb_commit_transaction(db_ptr, &mut err_ptr);
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());

            let query = CString::new("MATCH (n) RETURN n").unwrap();
            let mut out_json: *mut c_char = ptr::null_mut();
            let status = nervusdb_exec_cypher(
                db_ptr,
                query.as_ptr(),
                ptr::null(),
                &mut out_json,
                &mut err_ptr,
            );
            assert_eq!(status, NERVUSDB_OK);
            assert!(err_ptr.is_null());
            assert!(!out_json.is_null());
            let json = CStr::from_ptr(out_json).to_string_lossy().to_string();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            nervusdb_free_string(out_json);

            nervusdb_close(db_ptr);
            if !err_ptr.is_null() {
                nervusdb_free_error(err_ptr);
            }
        }
    }

    fn intern(db: *mut nervusdb_db, value: &str, err: &mut *mut nervusdb_error) -> u64 {
        let c_value = CString::new(value).unwrap();
        let mut out = 0u64;
        let status = unsafe { nervusdb_intern(db, c_value.as_ptr(), &mut out, err) };
        assert_eq!(status, NERVUSDB_OK);
        out
    }
}
