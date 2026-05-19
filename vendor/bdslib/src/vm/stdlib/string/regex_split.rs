extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use fancy_regex::Regex;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[time_graph::instrument]
pub fn string_regex_split_base(
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
                    Ok(pattern) => match Regex::new(&pattern) {
                        Ok(regx) => {
                            let mut res = Value::list();
                            let fields: Vec<&str> =
                                regx.split(&string_data).map(|x| x.unwrap()).collect();
                            for f in fields {
                                res = res.push(Value::from_string(&f));
                            }
                            vm.stack.push(res);
                        }
                        Err(err) => {
                            bail!("{} compile returns: {}", &err_prefix, err);
                        }
                    },
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

pub fn stdlib_string_stack_regex_split_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_regex_split_base(vm, StackOps::FromStack, "STRING.REGEX.SPLIT".to_string())
}

pub fn stdlib_string_workbench_regex_split_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_regex_split_base(
        vm,
        StackOps::FromWorkBench,
        "STRING.REGEX.SPLIT.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.regex.split".to_string(),
        stdlib_string_stack_regex_split_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.regex.split.".to_string(),
        stdlib_string_workbench_regex_split_inline,
    )?;

    Ok(())
}
