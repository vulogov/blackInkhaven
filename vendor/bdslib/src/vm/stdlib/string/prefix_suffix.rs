extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum PSOps {
    Prefix,
    Suffix,
}

#[time_graph::instrument]
fn string_is_ps_base(
    vm: &mut VM,
    op: StackOps,
    psop: PSOps,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 2 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.workbench.len() < 1 {
                bail!("Workbench is too shallow for inline {}", &err_prefix);
            }
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
    }
    let string_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let pattern_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull(),
    };
    match string_val {
        Some(string_val) => match string_val.cast_string() {
            Ok(string_data) => match pattern_val {
                Some(pattern_val) => match pattern_val.cast_string() {
                    Ok(pattern) => {
                        let res = match psop {
                            PSOps::Prefix => string_data.starts_with(&pattern),
                            PSOps::Suffix => string_data.ends_with(&pattern),
                        };
                        vm.stack.push(Value::from_bool(res));
                    }
                    Err(err) => {
                        bail!("{} returns: {}", &err_prefix, err);
                    }
                },
                None => {
                    bail!("{} returns: NO DATA #2", &err_prefix);
                }
            },
            Err(err) => {
                bail!("{} returns: {}", &err_prefix, err);
            }
        },
        None => {
            bail!("{} returns: NO DATA #1", &err_prefix);
        }
    }
    Ok(vm)
}

pub fn stdlib_string_stack_is_ps_prefix_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_is_ps_base(
        vm,
        StackOps::FromStack,
        PSOps::Prefix,
        "STRING.PREFIX".to_string(),
    )
}

pub fn stdlib_string_workbench_is_ps_prefix_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_is_ps_base(
        vm,
        StackOps::FromWorkBench,
        PSOps::Prefix,
        "STRING.PREFIX.".to_string(),
    )
}

pub fn stdlib_string_stack_is_ps_suffix_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_is_ps_base(
        vm,
        StackOps::FromStack,
        PSOps::Suffix,
        "STRING.SUFFIX".to_string(),
    )
}

pub fn stdlib_string_workbench_is_ps_suffix_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_is_ps_base(
        vm,
        StackOps::FromWorkBench,
        PSOps::Suffix,
        "STRING.SUFFIX.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.prefix".to_string(),
        stdlib_string_stack_is_ps_prefix_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.prefix.".to_string(),
        stdlib_string_workbench_is_ps_prefix_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.suffix".to_string(),
        stdlib_string_stack_is_ps_suffix_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.suffix.".to_string(),
        stdlib_string_workbench_is_ps_suffix_inline,
    )?;

    Ok(())
}
