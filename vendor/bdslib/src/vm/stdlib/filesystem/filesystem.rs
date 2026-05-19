extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

use crate::vm::helpers;

#[derive(Debug, Clone)]
pub enum FsysOperations {
    IsFile,
}

pub fn bund_filesystem_base(
    vm: &mut VM,
    op: StackOps,
    fop: FsysOperations,
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
    let fn_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    match fn_val {
        Some(fn_val) => match fn_val.cast_string() {
            Ok(fn_name) => {
                let _ = match fop {
                    FsysOperations::IsFile => vm.stack.push(Value::from_bool(
                        helpers::filesystem_helper::filesystem_is_file(fn_name),
                    )),
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

pub fn stdlib_fs_is_file_from_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_filesystem_base(
        vm,
        StackOps::FromStack,
        FsysOperations::IsFile,
        "FS.IS_FILE".to_string(),
    )
}
pub fn stdlib_fs_is_file_from_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_filesystem_base(
        vm,
        StackOps::FromStack,
        FsysOperations::IsFile,
        "FS.IS_FILE".to_string(),
    )
}

pub fn stdlib_bund_filesystem_disabled(_vm: &mut VM) -> Result<&mut VM, Error> {
    bail!("bund FILESYSTEM functions disabled with --noio");
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "fs.is_file".to_string(),
        stdlib_fs_is_file_from_stack_inline,
    );
    let _ = vm
        .vm
        .register_inline("fs_is_file.".to_string(), stdlib_fs_is_file_from_wb_inline);
    Ok(())
}
