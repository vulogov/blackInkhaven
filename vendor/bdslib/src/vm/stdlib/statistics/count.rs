extern crate log;

use crate::vm::stdlib::statistics;
use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[time_graph::instrument]
fn stats_count_base(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match statistics::get_data::get_data(vm, op.clone(), smode, err_prefix.clone()) {
        Ok(res) => {
            let _ = match op {
                StackOps::FromStack => vm.stack.push(Value::from_int(res.len() as i64)),
                StackOps::FromWorkBench => vm
                    .stack
                    .push_to_workbench(Value::from_int(res.len() as i64)),
            };
        }
        Err(err) => {
            bail!("{} returned: {}", &err_prefix, err);
        }
    }
    Ok(vm)
}

pub fn stdlib_stats_stack_consume_count(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_count_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        "STAT.COUNT".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_count(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_count_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        "STAT.COUNT.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_count(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_count_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        "STAT.COUNT,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_count(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_count_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        "STAT.COUNT.,".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("stat.count".to_string(), stdlib_stats_stack_consume_count)?;
    let _ = vm
        .vm
        .register_inline("stat.count.".to_string(), stdlib_stats_wb_consume_count)?;
    let _ = vm
        .vm
        .register_inline("stat.count,".to_string(), stdlib_stats_stack_keep_count)?;
    let _ = vm
        .vm
        .register_inline("stat.count.,".to_string(), stdlib_stats_wb_keep_count)?;

    Ok(())
}
