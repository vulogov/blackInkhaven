extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::dynamic_to_json;
use super::doc_helpers::{pull, push, require_depth, value_to_uuid, value_to_string, value_to_f32_vec};

// ── doc.update.metadata / doc.update.metadata. ───────────────────────────────
// Stack: id(STRING)  metadata(MAP)
//        TOS-1        TOS

fn doc_update_metadata_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let metadata_val = pull(vm, &op).unwrap();
    let id_val       = pull(vm, &op).unwrap();
    let metadata = dynamic_to_json(metadata_val);
    let id       = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    match db.doc_update_metadata(id, metadata) {
        Ok(())   => push(vm, &op, Value::from_bool(true)),
        Err(err) => bail!("{} doc_update_metadata returned: {}", err_prefix, err),
    }
    Ok(vm)
}

pub fn stdlib_doc_update_metadata_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_update_metadata_base(vm, StackOps::FromStack, "DOC.UPDATE.METADATA")
}
pub fn stdlib_doc_update_metadata_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_update_metadata_base(vm, StackOps::FromWorkBench, "DOC.UPDATE.METADATA.")
}

// ── doc.update.content / doc.update.content. ─────────────────────────────────
// Stack: id(STRING)  content(STRING)
//        TOS-1        TOS

fn doc_update_content_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let content_val = pull(vm, &op).unwrap();
    let id_val      = pull(vm, &op).unwrap();
    let content = value_to_string(content_val, "content", err_prefix)?;
    let id      = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    match db.doc_update_content(id, content.as_bytes()) {
        Ok(())   => push(vm, &op, Value::from_bool(true)),
        Err(err) => bail!("{} doc_update_content returned: {}", err_prefix, err),
    }
    Ok(vm)
}

pub fn stdlib_doc_update_content_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_update_content_base(vm, StackOps::FromStack, "DOC.UPDATE.CONTENT")
}
pub fn stdlib_doc_update_content_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_update_content_base(vm, StackOps::FromWorkBench, "DOC.UPDATE.CONTENT.")
}

// ── doc.store.meta.vec / doc.store.meta.vec. ─────────────────────────────────
// Stack: id(STRING)  meta_vec(LIST)  metadata(MAP)
//        TOS-2        TOS-1           TOS

fn doc_store_meta_vec_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 3, err_prefix)?;
    let metadata_val = pull(vm, &op).unwrap();
    let meta_vec_val = pull(vm, &op).unwrap();
    let id_val       = pull(vm, &op).unwrap();
    let metadata = dynamic_to_json(metadata_val);
    let meta_vec = value_to_f32_vec(meta_vec_val, err_prefix)?;
    let id       = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    match db.doc_store_metadata_vector(id, meta_vec, metadata) {
        Ok(())   => push(vm, &op, Value::from_bool(true)),
        Err(err) => bail!("{} doc_store_metadata_vector returned: {}", err_prefix, err),
    }
    Ok(vm)
}

pub fn stdlib_doc_store_meta_vec_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_store_meta_vec_base(vm, StackOps::FromStack, "DOC.STORE.META.VEC")
}
pub fn stdlib_doc_store_meta_vec_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_store_meta_vec_base(vm, StackOps::FromWorkBench, "DOC.STORE.META.VEC.")
}

// ── doc.store.content.vec / doc.store.content.vec. ───────────────────────────
// Stack: id(STRING)  content_vec(LIST)
//        TOS-1        TOS

fn doc_store_content_vec_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let content_vec_val = pull(vm, &op).unwrap();
    let id_val          = pull(vm, &op).unwrap();
    let content_vec = value_to_f32_vec(content_vec_val, err_prefix)?;
    let id          = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    match db.doc_store_content_vector(id, content_vec) {
        Ok(())   => push(vm, &op, Value::from_bool(true)),
        Err(err) => bail!("{} doc_store_content_vector returned: {}", err_prefix, err),
    }
    Ok(vm)
}

pub fn stdlib_doc_store_content_vec_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_store_content_vec_base(vm, StackOps::FromStack, "DOC.STORE.CONTENT.VEC")
}
pub fn stdlib_doc_store_content_vec_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_store_content_vec_base(vm, StackOps::FromWorkBench, "DOC.STORE.CONTENT.VEC.")
}

// ── registration ─────────────────────────────────────────────────────────────

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.update.metadata".to_string(),    stdlib_doc_update_metadata_stack)?;
    let _ = vm.vm.register_inline("doc.update.metadata.".to_string(),   stdlib_doc_update_metadata_workbench)?;
    let _ = vm.vm.register_inline("doc.update.content".to_string(),     stdlib_doc_update_content_stack)?;
    let _ = vm.vm.register_inline("doc.update.content.".to_string(),    stdlib_doc_update_content_workbench)?;
    let _ = vm.vm.register_inline("doc.store.meta.vec".to_string(),     stdlib_doc_store_meta_vec_stack)?;
    let _ = vm.vm.register_inline("doc.store.meta.vec.".to_string(),    stdlib_doc_store_meta_vec_workbench)?;
    let _ = vm.vm.register_inline("doc.store.content.vec".to_string(),  stdlib_doc_store_content_vec_stack)?;
    let _ = vm.vm.register_inline("doc.store.content.vec.".to_string(), stdlib_doc_store_content_vec_workbench)?;
    Ok(())
}
