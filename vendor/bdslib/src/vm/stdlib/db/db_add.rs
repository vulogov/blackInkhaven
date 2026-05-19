extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers::eval::dynamic_to_json;

fn db_add_base<'a>(vm: &'a mut VM, op: StackOps, err_prefix: &str) -> Result<&'a mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.workbench.len() < 1 {
                bail!("Workbench is too shallow for inline {}", err_prefix);
            }
        }
    }
    let doc_val = match op {
        StackOps::FromStack     => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let doc_val = match doc_val {
        Some(v) => v,
        None    => bail!("{} returns NO DATA", err_prefix),
    };
    let doc_json = dynamic_to_json(doc_val);
    let db = match crate::globals::get_db() {
        Ok(db)   => db,
        Err(err) => bail!("{} cannot access global DB: {}", err_prefix, err),
    };
    let id = match db.add(doc_json) {
        Ok(id)   => id,
        Err(err) => bail!("{} db.add returned: {}", err_prefix, err),
    };
    let result = Value::from_string(id.to_string());
    let _ = match op {
        StackOps::FromStack     => vm.stack.push(result),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(result),
    };
    Ok(vm)
}

pub fn stdlib_db_add_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    db_add_base(vm, StackOps::FromStack, "DB.ADD")
}

pub fn stdlib_db_add_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    db_add_base(vm, StackOps::FromWorkBench, "DB.ADD.")
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("db.add".to_string(),  stdlib_db_add_stack)?;
    let _ = vm.vm.register_inline("db.add.".to_string(), stdlib_db_add_workbench)?;
    Ok(())
}
