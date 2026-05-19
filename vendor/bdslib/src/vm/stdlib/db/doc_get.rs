extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::json_to_dynamic;
use super::doc_helpers::{pull, push, require_depth, value_to_uuid};

// ── doc.get.metadata / doc.get.metadata. ─────────────────────────────────────
// Stack: id(STRING)
//        TOS
// Pushes the metadata MAP, or null if the document does not exist.

fn doc_get_metadata_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 1, err_prefix)?;
    let id_val = pull(vm, &op).unwrap();
    let id     = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let result = match db.doc_get_metadata(id) {
        Ok(Some(meta)) => json_to_dynamic(meta),
        Ok(None)       => Value::nodata(),
        Err(err)       => bail!("{} doc_get_metadata returned: {}", err_prefix, err),
    };
    push(vm, &op, result);
    Ok(vm)
}

pub fn stdlib_doc_get_metadata_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_get_metadata_base(vm, StackOps::FromStack, "DOC.GET.METADATA")
}
pub fn stdlib_doc_get_metadata_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_get_metadata_base(vm, StackOps::FromWorkBench, "DOC.GET.METADATA.")
}

// ── doc.get.content / doc.get.content. ───────────────────────────────────────
// Stack: id(STRING)
//        TOS
// Pushes the content as a STRING (UTF-8), or null if the document does not exist.

fn doc_get_content_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 1, err_prefix)?;
    let id_val = pull(vm, &op).unwrap();
    let id     = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let result = match db.doc_get_content(id) {
        Ok(Some(bytes)) => Value::from_string(String::from_utf8_lossy(&bytes).into_owned()),
        Ok(None)        => Value::nodata(),
        Err(err)        => bail!("{} doc_get_content returned: {}", err_prefix, err),
    };
    push(vm, &op, result);
    Ok(vm)
}

pub fn stdlib_doc_get_content_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_get_content_base(vm, StackOps::FromStack, "DOC.GET.CONTENT")
}
pub fn stdlib_doc_get_content_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_get_content_base(vm, StackOps::FromWorkBench, "DOC.GET.CONTENT.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.get.metadata".to_string(),  stdlib_doc_get_metadata_stack)?;
    let _ = vm.vm.register_inline("doc.get.metadata.".to_string(), stdlib_doc_get_metadata_workbench)?;
    let _ = vm.vm.register_inline("doc.get.content".to_string(),   stdlib_doc_get_content_stack)?;
    let _ = vm.vm.register_inline("doc.get.content.".to_string(),  stdlib_doc_get_content_workbench)?;
    Ok(())
}
