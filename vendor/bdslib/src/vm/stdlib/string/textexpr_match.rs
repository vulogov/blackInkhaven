extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use srch;

#[time_graph::instrument]
pub fn string_srch_match_base(
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
    let string1_val_get = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let string2_val_get = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull(),
    };

    let string1_val = match string1_val_get {
        Some(string1_val) => string1_val,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let string2_val = match string2_val_get {
        Some(string2_val) => string2_val,
        None => {
            bail!("{} returns NO DATA #2", &err_prefix);
        }
    };

    let string1_data = match string1_val.cast_string() {
        Ok(string1_data) => string1_data,
        Err(err) => {
            bail!("{} returned for #1: {}", &err_prefix, err);
        }
    };
    let string2_data = match string2_val.cast_string() {
        Ok(string2_data) => string2_data,
        Err(err) => {
            bail!("{} returned for #2: {}", &err_prefix, err);
        }
    };
    let matcher = match srch::Expression::new(&string1_data) {
        Ok(matcher) => matcher,
        Err(err) => bail!(
            "{} returned error when creates matcher: {:?}",
            &err_prefix,
            err
        ),
    };
    let _ = match op {
        StackOps::FromStack => vm
            .stack
            .push(Value::from_bool(matcher.matches(string2_data))),
        StackOps::FromWorkBench => vm
            .stack
            .push_to_workbench(Value::from_bool(matcher.matches(string2_data))),
    };
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_string_stack_srch_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_srch_match_base(
        vm,
        StackOps::FromStack,
        "STRING.EXPRESSIONMATCH".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_string_workbench_srch_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_srch_match_base(
        vm,
        StackOps::FromWorkBench,
        "STRING.EXPRESSIONMATCH.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.expressionmatch".to_string(),
        stdlib_string_stack_srch_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.expressionmatch.".to_string(),
        stdlib_string_workbench_srch_inline,
    )?;

    Ok(())
}
