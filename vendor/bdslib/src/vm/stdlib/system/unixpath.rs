extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use unix_path;

#[derive(Debug, Clone)]
pub enum UnixPathAlgorithm {
    Split,
    Filename,
}

fn string_system_path_split(vm: &mut VM, op: StackOps, name: String) -> Result<&mut VM, Error> {
    let mut res = Value::list();
    let path_iter = unix_path::Path::new(&name).iter();
    for p in path_iter {
        res = res.push(Value::from_string(p.to_str().unwrap()));
    }
    match op {
        StackOps::FromStack => vm.stack.push(res),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
    };
    Ok(vm)
}

fn string_system_path_filename(vm: &mut VM, op: StackOps, name: String) -> Result<&mut VM, Error> {
    let path = unix_path::Path::new(&name).file_name();
    match path {
        Some(path) => {
            let res = Value::from_string(path.to_str().unwrap());
            match op {
                StackOps::FromStack => vm.stack.push(res),
                StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
            };
        }
        None => {
            bail!("Error getting filename for: {}", &name);
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn string_system_path_base(
    vm: &mut VM,
    op: StackOps,
    pop: UnixPathAlgorithm,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.workbench.len() < 1 {
                bail!("Workbench is too shallow for inline {}", &err_prefix);
            }
        }
    }
    let up_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };

    let up_value = match up_val {
        Some(up_value) => up_value,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };

    let name = match up_value.cast_string() {
        Ok(name) => name,
        Err(err) => {
            bail!("{} returned for #1: {}", &err_prefix, err);
        }
    };
    return match pop {
        UnixPathAlgorithm::Split => string_system_path_split(vm, op.clone(), name),
        UnixPathAlgorithm::Filename => string_system_path_filename(vm, op.clone(), name),
    };
}

pub fn stdlib_system_path_split_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    string_system_path_base(
        vm,
        StackOps::FromStack,
        UnixPathAlgorithm::Split,
        "SYSTEM.PATH.SPLIT".to_string(),
    )
}
pub fn stdlib_system_path_split_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    string_system_path_base(
        vm,
        StackOps::FromWorkBench,
        UnixPathAlgorithm::Split,
        "SYSTEM.PATH.SPLIT.".to_string(),
    )
}

pub fn stdlib_system_path_filename_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    string_system_path_base(
        vm,
        StackOps::FromStack,
        UnixPathAlgorithm::Filename,
        "SYSTEM.PATH.FILENAME".to_string(),
    )
}
pub fn stdlib_system_path_filename_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    string_system_path_base(
        vm,
        StackOps::FromWorkBench,
        UnixPathAlgorithm::Filename,
        "SYSTEM.PATH.FILENAME.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "system.path.split".to_string(),
        stdlib_system_path_split_stack,
    );
    let _ = vm.vm.register_inline(
        "system.path.split.".to_string(),
        stdlib_system_path_split_workbench,
    );
    let _ = vm.vm.register_inline(
        "system.path.filename".to_string(),
        stdlib_system_path_filename_stack,
    );
    let _ = vm.vm.register_inline(
        "system.path.filename.".to_string(),
        stdlib_system_path_filename_workbench,
    );
    Ok(())
}
