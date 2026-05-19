extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use fs_extra::dir;
use rust_dynamic::types::*;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

#[derive(Debug, Clone)]
pub enum FsOperations {
    Copy,
    Move,
    Remove,
}

fn remove_items(t: &Vec<String>) -> Result<u64, fs_extra::error::Error> {
    match fs_extra::remove_items(t) {
        Ok(_) => {
            return Ok(0);
        }
        Err(err) => {
            return Err(err);
        }
    }
}

pub fn stdlib_bund_file_cp_base(
    vm: &mut VM,
    op: FsOperations,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    let options = dir::CopyOptions::new();
    let mut from_paths: Vec<String> = Vec::new();

    match op {
        FsOperations::Remove => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
        _ => {
            if vm.stack.current_stack_len() < 2 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
    };

    let file_name_value = match vm.stack.pull() {
        Some(file_name) => file_name,
        None => {
            bail!("{} NO DATA #1", &err_prefix)
        }
    };

    let target_file_name_value = match op {
        FsOperations::Remove => Value::from_string(""),
        _ => match vm.stack.pull() {
            Some(file_name) => file_name,
            None => {
                bail!("{} NO DATA #2", &err_prefix)
            }
        },
    };

    let target_file_name = match target_file_name_value.cast_string() {
        Ok(target_file_name) => target_file_name,
        Err(err) => {
            bail!("Error casting string for target {}: {}", &err_prefix, err);
        }
    };
    match file_name_value.type_of() {
        STRING => {
            let file_name = match file_name_value.cast_string() {
                Ok(file_name) => file_name,
                Err(err) => {
                    bail!("Error casting string for {}: {}", &err_prefix, err);
                }
            };
            from_paths.push(file_name.clone());
        }
        LIST => match file_name_value.cast_list() {
            Ok(file_name_list) => {
                for v in file_name_list {
                    match v.cast_string() {
                        Ok(file_name) => {
                            from_paths.push(file_name.clone());
                        }
                        Err(err) => {
                            bail!("Error casting string for {}: {}", &err_prefix, err);
                        }
                    }
                }
            }
            Err(err) => {
                bail!("Error casting list for {}: {}", &err_prefix, err);
            }
        },
        _ => {
            bail!("Incorrect #1 type for {}", &err_prefix);
        }
    }
    log::debug!(
        "{} src: {:?} target: {}",
        &err_prefix,
        &from_paths,
        &target_file_name
    );
    let res = match op {
        FsOperations::Copy => fs_extra::copy_items(&from_paths, target_file_name.clone(), &options),
        FsOperations::Move => fs_extra::move_items(&from_paths, target_file_name.clone(), &options),
        FsOperations::Remove => remove_items(&from_paths),
    };
    match res {
        Ok(_) => {
            vm.stack.push(Value::from_bool(true));
        }
        Err(err) => {
            bail!("{} returns: {}", &err_prefix, err);
        }
    }
    Ok(vm)
}

pub fn stdlib_bund_file_cp_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_file_cp_base(vm, FsOperations::Copy, "FS.CP".to_string())
}

pub fn stdlib_bund_file_mv_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_file_cp_base(vm, FsOperations::Move, "FS.MV".to_string())
}

pub fn stdlib_bund_file_rm_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_file_cp_base(vm, FsOperations::Remove, "FS.RM".to_string())
}

pub fn stdlib_bund_file_cp_disabled(_vm: &mut VM) -> Result<&mut VM, Error> {
    bail!("bund FS.CP functions disabled with --noio");
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("fs.cp".to_string(), stdlib_bund_file_cp_inline);
    let _ = vm
        .vm
        .register_inline("fs.mv".to_string(), stdlib_bund_file_mv_inline);
    let _ = vm
        .vm
        .register_inline("fs.rm".to_string(), stdlib_bund_file_rm_inline);
    Ok(())
}
