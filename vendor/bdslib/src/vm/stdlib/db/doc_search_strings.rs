extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::dynamic_to_json;
use super::doc_helpers::{pull, push, require_depth, value_to_string, value_to_f32_vec};

// ── doc.search.strings / doc.search.strings. ─────────────────────────────────
// Calls doc_search_text_strings: text query → list of fingerprint strings.
// Stack: query(STRING)  limit(INT)
//        TOS-1           TOS

fn doc_search_strings_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let limit_val = pull(vm, &op).unwrap();
    let query_val = pull(vm, &op).unwrap();
    let limit = match limit_val.cast_int() {
        Ok(i)    => i as usize,
        Err(err) => bail!("{} limit cast failed: {}", err_prefix, err),
    };
    let query = value_to_string(query_val, "query", err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let hits = match db.doc_search_text_strings(&query, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search_text_strings returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(Value::from_string).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_strings_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_strings_base(vm, StackOps::FromStack, "DOC.SEARCH.STRINGS")
}
pub fn stdlib_doc_search_strings_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_strings_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.STRINGS.")
}

// ── doc.search.json.strings / doc.search.json.strings. ───────────────────────
// Calls doc_search_json_strings: JSON query MAP → list of fingerprint strings.
// Stack: query(MAP)  limit(INT)
//        TOS-1        TOS

fn doc_search_json_strings_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let limit_val = pull(vm, &op).unwrap();
    let query_val = pull(vm, &op).unwrap();
    let limit      = match limit_val.cast_int() {
        Ok(i)    => i as usize,
        Err(err) => bail!("{} limit cast failed: {}", err_prefix, err),
    };
    let query_json = dynamic_to_json(query_val);
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let hits = match db.doc_search_json_strings(&query_json, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search_json_strings returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(Value::from_string).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_json_strings_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_json_strings_base(vm, StackOps::FromStack, "DOC.SEARCH.JSON.STRINGS")
}
pub fn stdlib_doc_search_json_strings_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_json_strings_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.JSON.STRINGS.")
}

// ── doc.search.vec.strings / doc.search.vec.strings. ─────────────────────────
// Calls doc_search_strings: pre-computed vector → list of fingerprint strings.
// Stack: query_vec(LIST of floats)  limit(INT)
//        TOS-1                       TOS

fn doc_search_vec_strings_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let limit_val     = pull(vm, &op).unwrap();
    let query_vec_val = pull(vm, &op).unwrap();
    let limit     = match limit_val.cast_int() {
        Ok(i)    => i as usize,
        Err(err) => bail!("{} limit cast failed: {}", err_prefix, err),
    };
    let query_vec = value_to_f32_vec(query_vec_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let hits = match db.doc_search_strings(query_vec, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search_strings returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(Value::from_string).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_vec_strings_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_vec_strings_base(vm, StackOps::FromStack, "DOC.SEARCH.VEC.STRINGS")
}
pub fn stdlib_doc_search_vec_strings_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_vec_strings_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.VEC.STRINGS.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.search.strings".to_string(),          stdlib_doc_search_strings_stack)?;
    let _ = vm.vm.register_inline("doc.search.strings.".to_string(),         stdlib_doc_search_strings_workbench)?;
    let _ = vm.vm.register_inline("doc.search.json.strings".to_string(),     stdlib_doc_search_json_strings_stack)?;
    let _ = vm.vm.register_inline("doc.search.json.strings.".to_string(),    stdlib_doc_search_json_strings_workbench)?;
    let _ = vm.vm.register_inline("doc.search.vec.strings".to_string(),      stdlib_doc_search_vec_strings_stack)?;
    let _ = vm.vm.register_inline("doc.search.vec.strings.".to_string(),     stdlib_doc_search_vec_strings_workbench)?;
    Ok(())
}
