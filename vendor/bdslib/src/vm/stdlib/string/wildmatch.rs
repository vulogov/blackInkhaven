extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use wildmatch::WildMatch;

#[time_graph::instrument]
pub fn string_wildcard_base(
    vm: &mut VM,
    op: StackOps,
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
                        if WildMatch::new(&pattern).matches(&string_data) {
                            vm.stack.push(Value::from_bool(true));
                        } else {
                            vm.stack.push(Value::from_bool(false));
                        }
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

pub fn stdlib_string_stack_wildcard_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_wildcard_base(vm, StackOps::FromStack, "STRING.WILDCARD".to_string())
}

pub fn stdlib_string_workbench_wildcard_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_wildcard_base(vm, StackOps::FromWorkBench, "STRING.WILDCARD.".to_string())
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.wildcard".to_string(),
        stdlib_string_stack_wildcard_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.wildcard.".to_string(),
        stdlib_string_workbench_wildcard_inline,
    )?;

    Ok(())
}
