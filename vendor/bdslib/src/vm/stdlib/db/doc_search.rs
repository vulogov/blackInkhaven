extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::{dynamic_to_json, json_to_dynamic};
use super::doc_helpers::{pull, push, require_depth, value_to_string, value_to_f32_vec};

// ── doc.search / doc.search. ─────────────────────────────────────────────────
// Calls doc_search_text: embeds `query` string and returns ranked JSON docs.
// Stack: query(STRING)  limit(INT)
//        TOS-1           TOS

fn doc_search_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
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
    let hits = match db.doc_search_text(&query, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search_text returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(json_to_dynamic).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_base(vm, StackOps::FromStack, "DOC.SEARCH")
}
pub fn stdlib_doc_search_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.")
}

// ── doc.search.json / doc.search.json. ───────────────────────────────────────
// Calls doc_search_json: fingerprints the query MAP and embeds it.
// Stack: query(MAP)  limit(INT)
//        TOS-1        TOS

fn doc_search_json_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
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
    let hits = match db.doc_search_json(&query_json, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search_json returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(json_to_dynamic).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_json_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_json_base(vm, StackOps::FromStack, "DOC.SEARCH.JSON")
}
pub fn stdlib_doc_search_json_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_json_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.JSON.")
}

// ── doc.search.vec / doc.search.vec. ─────────────────────────────────────────
// Calls doc_search: raw pre-computed query vector.
// Stack: query_vec(LIST of floats)  limit(INT)
//        TOS-1                       TOS

fn doc_search_vec_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
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
    let hits = match db.doc_search(query_vec, limit) {
        Ok(hits) => hits,
        Err(err) => bail!("{} doc_search returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_list(hits.into_iter().map(json_to_dynamic).collect()));
    Ok(vm)
}

pub fn stdlib_doc_search_vec_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_vec_base(vm, StackOps::FromStack, "DOC.SEARCH.VEC")
}
pub fn stdlib_doc_search_vec_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_search_vec_base(vm, StackOps::FromWorkBench, "DOC.SEARCH.VEC.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.search".to_string(),       stdlib_doc_search_stack)?;
    let _ = vm.vm.register_inline("doc.search.".to_string(),      stdlib_doc_search_workbench)?;
    let _ = vm.vm.register_inline("doc.search.json".to_string(),  stdlib_doc_search_json_stack)?;
    let _ = vm.vm.register_inline("doc.search.json.".to_string(), stdlib_doc_search_json_workbench)?;
    let _ = vm.vm.register_inline("doc.search.vec".to_string(),   stdlib_doc_search_vec_stack)?;
    let _ = vm.vm.register_inline("doc.search.vec.".to_string(),  stdlib_doc_search_vec_workbench)?;
    Ok(())
}
