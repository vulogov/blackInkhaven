extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::dynamic_to_json;
use super::doc_helpers::{pull, push, require_depth, value_to_f32_vec, value_to_string};

// ── doc.add / doc.add. ────────────────────────────────────────────────────────
// Stack: metadata(MAP)  content(STRING)
//        TOS-1           TOS

fn doc_add_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let content_val  = pull(vm, &op).unwrap();
    let metadata_val = pull(vm, &op).unwrap();
    let content  = value_to_string(content_val, "content", err_prefix)?;
    let metadata = dynamic_to_json(metadata_val);
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let id = match db.doc_add(metadata, content.as_bytes()) {
        Ok(id)   => id,
        Err(err) => bail!("{} doc_add returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_string(id.to_string()));
    Ok(vm)
}

pub fn stdlib_doc_add_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_base(vm, StackOps::FromStack, "DOC.ADD")
}
pub fn stdlib_doc_add_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_base(vm, StackOps::FromWorkBench, "DOC.ADD.")
}

// ── doc.add.file / doc.add.file. ─────────────────────────────────────────────
// Stack: path(STRING)  name(STRING)  slice(INT)  overlap(FLOAT)
//        TOS-3          TOS-2          TOS-1       TOS

fn doc_add_file_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 4, err_prefix)?;
    let overlap_val = pull(vm, &op).unwrap();
    let slice_val   = pull(vm, &op).unwrap();
    let name_val    = pull(vm, &op).unwrap();
    let path_val    = pull(vm, &op).unwrap();
    let overlap = match overlap_val.cast_float() {
        Ok(f)    => f as f32,
        Err(err) => bail!("{} overlap cast failed: {}", err_prefix, err),
    };
    let slice = match slice_val.cast_int() {
        Ok(i)    => i as usize,
        Err(err) => bail!("{} slice cast failed: {}", err_prefix, err),
    };
    let name = value_to_string(name_val, "name", err_prefix)?;
    let path = value_to_string(path_val, "path", err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let id = match db.doc_add_from_file(&path, &name, slice, overlap) {
        Ok(id)   => id,
        Err(err) => bail!("{} doc_add_from_file returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_string(id.to_string()));
    Ok(vm)
}

pub fn stdlib_doc_add_file_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_file_base(vm, StackOps::FromStack, "DOC.ADD.FILE")
}
pub fn stdlib_doc_add_file_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_file_base(vm, StackOps::FromWorkBench, "DOC.ADD.FILE.")
}

// ── doc.add.vec / doc.add.vec. ───────────────────────────────────────────────
// Stack: metadata(MAP)  content(STRING)  meta_vec(LIST)  content_vec(LIST)
//        TOS-3           TOS-2            TOS-1            TOS

fn doc_add_vec_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 4, err_prefix)?;
    let content_vec_val = pull(vm, &op).unwrap();
    let meta_vec_val    = pull(vm, &op).unwrap();
    let content_val     = pull(vm, &op).unwrap();
    let metadata_val    = pull(vm, &op).unwrap();
    let content_vec = value_to_f32_vec(content_vec_val, err_prefix)?;
    let meta_vec    = value_to_f32_vec(meta_vec_val, err_prefix)?;
    let content     = value_to_string(content_val, "content", err_prefix)?;
    let metadata    = dynamic_to_json(metadata_val);
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let id = match db.doc_add_with_vectors(metadata, content.as_bytes(), meta_vec, content_vec) {
        Ok(id)   => id,
        Err(err) => bail!("{} doc_add_with_vectors returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_string(id.to_string()));
    Ok(vm)
}

pub fn stdlib_doc_add_vec_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_vec_base(vm, StackOps::FromStack, "DOC.ADD.VEC")
}
pub fn stdlib_doc_add_vec_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_add_vec_base(vm, StackOps::FromWorkBench, "DOC.ADD.VEC.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.add".to_string(),       stdlib_doc_add_stack)?;
    let _ = vm.vm.register_inline("doc.add.".to_string(),      stdlib_doc_add_workbench)?;
    let _ = vm.vm.register_inline("doc.add.file".to_string(),  stdlib_doc_add_file_stack)?;
    let _ = vm.vm.register_inline("doc.add.file.".to_string(), stdlib_doc_add_file_workbench)?;
    let _ = vm.vm.register_inline("doc.add.vec".to_string(),   stdlib_doc_add_vec_stack)?;
    let _ = vm.vm.register_inline("doc.add.vec.".to_string(),  stdlib_doc_add_vec_workbench)?;
    Ok(())
}
