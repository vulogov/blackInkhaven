extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::json_to_dynamic;

// Stack layout before calling db.fulltext / db.fulltext.:
//   TOS      → duration string  (e.g. "1h")
//   TOS - 1  → query string     (e.g. "nginx error")

fn db_fulltext_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 2 {
                bail!("Stack is too shallow for inline {} (need query + duration)", err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.workbench.len() < 2 {
                bail!("Workbench is too shallow for inline {} (need query + duration)", err_prefix);
            }
        }
    }
    let duration_val = match op {
        StackOps::FromStack     => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let duration = match duration_val {
        Some(v) => match v.cast_string() {
            Ok(s)    => s,
            Err(err) => bail!("{} duration cast failed: {}", err_prefix, err),
        },
        None => bail!("{} returns NO DATA (duration)", err_prefix),
    };
    let query_val = match op {
        StackOps::FromStack     => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let query = match query_val {
        Some(v) => match v.cast_string() {
            Ok(s)    => s,
            Err(err) => bail!("{} query cast failed: {}", err_prefix, err),
        },
        None => bail!("{} returns NO DATA (query)", err_prefix),
    };
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let hits = match db.search_fts(&duration, &query) {
        Ok(hits) => hits,
        Err(err) => bail!("{} search_fts returned: {}", err_prefix, err),
    };
    let results = Value::from_list(hits.into_iter().map(json_to_dynamic).collect());
    let _ = match op {
        StackOps::FromStack     => vm.stack.push(results),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(results),
    };
    Ok(vm)
}

pub fn stdlib_db_fulltext_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    db_fulltext_base(vm, StackOps::FromStack, "DB.FULLTEXT")
}

pub fn stdlib_db_fulltext_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    db_fulltext_base(vm, StackOps::FromWorkBench, "DB.FULLTEXT.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("db.fulltext".to_string(),  stdlib_db_fulltext_stack)?;
    let _ = vm.vm.register_inline("db.fulltext.".to_string(), stdlib_db_fulltext_workbench)?;
    Ok(())
}
