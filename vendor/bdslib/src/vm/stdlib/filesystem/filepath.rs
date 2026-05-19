extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use std::path::Path;

pub fn bund_filename_base(vm: &mut VM, op: StackOps, err_prefix: String) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
            if vm.stack.workbench.len() < 1 {
                bail!("Workbench is too shallow for inline {}", &err_prefix);
            }
        }
    }
    let fn_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    match fn_val {
        Some(fn_val) => match fn_val.cast_string() {
            Ok(fn_name) => {
                match Path::new(&fn_name).canonicalize() {
                    Ok(p) => {
                        let fname = format!("{}", p.to_str().unwrap());
                        vm.stack.push(Value::from_string(fname));
                    }
                    Err(err) => {
                        bail!("{} returned: {}", &err_prefix, err);
                    }
                };
            }
            Err(err) => {
                bail!("{} returns: {}", &err_prefix, err);
            }
        },
        None => {
            bail!("{} returns: NO DATA", &err_prefix);
        }
    }
    Ok(vm)
}

pub fn stdlib_filename_from_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_filename_base(vm, StackOps::FromStack, "FILENAME".to_string())
}

pub fn stdlib_filename_from_workbech_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_filename_base(vm, StackOps::FromWorkBench, "FILENAME.".to_string())
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("filename".to_string(), stdlib_filename_from_stack_inline);
    let _ = vm.vm.register_inline(
        "filename.".to_string(),
        stdlib_filename_from_workbech_inline,
    );
    Ok(())
}
