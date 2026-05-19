extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use super::doc_helpers::push;

// ── doc.sync ─────────────────────────────────────────────────────────────────
// No stack arguments. Flushes the docstore HNSW index to disk.
// Pushes true on success.

pub fn stdlib_doc_sync(vm: &mut VM) -> Result<&mut VM, Error> {
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("DOC.SYNC cannot access global DB: {}", err),
    };
    match db.doc_sync() {
        Ok(())   => vm.stack.push(Value::from_bool(true)),
        Err(err) => bail!("DOC.SYNC returned: {}", err),
    };
    Ok(vm)
}

// ── doc.reindex / doc.reindex. ───────────────────────────────────────────────
// No stack arguments. Rebuilds the HNSW index from persisted stores.
// Pushes the number of documents indexed as an INT.

fn doc_reindex_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let count = match db.doc_reindex() {
        Ok(n)    => n,
        Err(err) => bail!("{} doc_reindex returned: {}", err_prefix, err),
    };
    push(vm, &op, Value::from_int(count as i64));
    Ok(vm)
}

pub fn stdlib_doc_reindex_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_reindex_base(vm, StackOps::FromStack, "DOC.REINDEX")
}
pub fn stdlib_doc_reindex_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    doc_reindex_base(vm, StackOps::FromWorkBench, "DOC.REINDEX.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("doc.sync".to_string(),      stdlib_doc_sync)?;
    let _ = vm.vm.register_inline("doc.reindex".to_string(),   stdlib_doc_reindex_stack)?;
    let _ = vm.vm.register_inline("doc.reindex.".to_string(),  stdlib_doc_reindex_workbench)?;
    Ok(())
}
