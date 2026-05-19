extern crate log;

use bundcore::bundcore::Bund;
use deunicode;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum UnicodeAlgorithm {
    Deunicode,
}

#[time_graph::instrument]
fn string_unicode_base(
    vm: &mut VM,
    op: StackOps,
    ta: UnicodeAlgorithm,
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
    let string_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    match string_val {
        Some(string_val) => {
            match string_val.cast_string() {
                Ok(string_data) => match ta {
                    UnicodeAlgorithm::Deunicode => {
                        match op {
                            StackOps::FromStack => vm
                                .stack
                                .push(Value::from_string(deunicode::deunicode(&string_data))),
                            StackOps::FromWorkBench => vm.stack.push_to_workbench(
                                Value::from_string(deunicode::deunicode(&string_data)),
                            ),
                        };
                    }
                },
                Err(err) => {
                    bail!("{} returns: {}", &err_prefix, err);
                }
            }
        }
        None => {
            bail!("{} returns: NO DATA #1", &err_prefix);
        }
    }
    Ok(vm)
}

pub fn stdlib_unicode_de_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_unicode_base(
        vm,
        StackOps::FromWorkBench,
        UnicodeAlgorithm::Deunicode,
        "STRING.DEUNICODE.".to_string(),
    )
}
pub fn stdlib_unicode_de_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_unicode_base(
        vm,
        StackOps::FromStack,
        UnicodeAlgorithm::Deunicode,
        "STRING.DEUNICODE".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.deunicode".to_string(),
        stdlib_unicode_de_stack_inline,
    )?;
    let _ = vm
        .vm
        .register_inline("string.deunicode.".to_string(), stdlib_unicode_de_wb_inline)?;

    Ok(())
}
