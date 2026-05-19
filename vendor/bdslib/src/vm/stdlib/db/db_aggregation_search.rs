extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::json_to_dynamic;
use super::doc_helpers::{pull, push, require_depth, value_to_string};

// ── db.aggregation.search / db.aggregation.search. ───────────────────────────
// Runs telemetry vector search and document-store semantic search concurrently,
// returning both result sets merged under a single MAP.
// Stack: query(STRING)  duration(STRING)
//        TOS-1           TOS

fn db_aggregation_search_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    require_depth(vm, &op, 2, err_prefix)?;
    let duration_val = pull(vm, &op).unwrap();
    let query_val    = pull(vm, &op).unwrap();
    let duration = value_to_string(duration_val, "duration", err_prefix)?;
    let query    = value_to_string(query_val,    "query",    err_prefix)?;
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let result = match db.aggregationsearch(&duration, &query) {
        Ok(v)    => json_to_dynamic(v),
        Err(err) => bail!("{} aggregationsearch returned: {}", err_prefix, err),
    };
    push(vm, &op, result);
    Ok(vm)
}

pub fn stdlib_db_aggregation_search_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    db_aggregation_search_base(vm, StackOps::FromStack, "DB.AGGREGATION.SEARCH")
}
pub fn stdlib_db_aggregation_search_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    db_aggregation_search_base(vm, StackOps::FromWorkBench, "DB.AGGREGATION.SEARCH.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("db.aggregation.search".to_string(),  stdlib_db_aggregation_search_stack)?;
    let _ = vm.vm.register_inline("db.aggregation.search.".to_string(), stdlib_db_aggregation_search_workbench)?;
    Ok(())
}
