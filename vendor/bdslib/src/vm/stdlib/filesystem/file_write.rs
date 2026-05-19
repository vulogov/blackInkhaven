extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_multistackvm::multistackvm::{StackOps, VM};

pub fn bund_file_write_base(
    vm: &mut VM,
    op: StackOps,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 2 {
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
    let file_name_value = match vm.stack.pull() {
        Some(file_name_value) => file_name_value,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let file_name = match file_name_value.cast_string() {
        Ok(file_name) => file_name,
        Err(err) => {
            bail!("{} returns NO CAST #1: {}", &err_prefix, err);
        }
    };

    let data_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };

    match data_val {
        Some(data_val) => match data_val.cast_string() {
            Ok(data) => match fs_extra::file::write_all(file_name, &data) {
                Ok(_) => {}
                Err(err) => {
                    bail!("{} returns: {}", &err_prefix, err);
                }
            },
            Err(err) => {
                bail!("{} returns: NO CAST #2: {}", &err_prefix, err);
            }
        },
        None => {
            bail!("{} returns: NO DATA #2", &err_prefix);
        }
    }
    Ok(vm)
}

pub fn stdlib_bund_file_write_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_file_write_base(vm, StackOps::FromStack, "FILE.WRITE".to_string())
}

pub fn stdlib_bund_file_write_from_wrokbench_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_file_write_base(vm, StackOps::FromWorkBench, "FILE.WRITE.".to_string())
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("file.write".to_string(), stdlib_bund_file_write_inline);
    let _ = vm.vm.register_inline(
        "file.write.".to_string(),
        stdlib_bund_file_write_from_wrokbench_inline,
    );
    Ok(())
}
