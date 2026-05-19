extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use super::doc_helpers::{pull, push, require_depth, value_to_uuid};

// ── doc.delete / doc.delete. ─────────────────────────────────────────────────
// Stack: id(STRING)
//        TOS
// Pushes true on success.

fn doc_delete_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 1, err_prefix)?;
    let id_val = pull(vm, &op).unwrap();
    let id     = value_to_uuid(id_val, err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    match db.doc_delete(id) {
        Ok(())   => push(vm, &op, Value::from_bool(true)),
        Err(err) => bail!("{} doc_delete returned: {}", err_prefix, err),
    }
    Ok(vm)
}

pub fn stdlib_doc_delete_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_delete_base(vm, StackOps::FromStack, "DOC.DELETE")
}
pub fn stdlib_doc_delete_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_delete_base(vm, StackOps::FromWorkBench, "DOC.DELETE.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.delete".to_string(),  stdlib_doc_delete_stack)?;
    let _ = vm.vm.register_inline("doc.delete.".to_string(), stdlib_doc_delete_workbench)?;
    Ok(())
}
