extern crate log;

use crate::vm::stdlib::statistics;
use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use ta::Next;
use ta::indicators::SimpleMovingAverage;

fn math_smoothing_base(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match statistics::get_data::get_data(vm, op.clone(), smode, err_prefix.clone()) {
        Ok(source) => {
            let mut res = Value::list();
            let mut sma = SimpleMovingAverage::new(3).unwrap();
            for val in source {
                let v = sma.next(val);
                res = res.push(Value::from_float(v));
            }
            let _ = match op {
                StackOps::FromStack => vm.stack.push(res),
                StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
            };
        }
        Err(err) => {
            bail!("{} returned: {}", &err_prefix, err);
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_math_stack_consume_smooth(vm: &mut VM) -> Result<&mut VM, Error> {
    math_smoothing_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        "MATH.SMOOTH".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_math_wb_consume_smooth(vm: &mut VM) -> Result<&mut VM, Error> {
    math_smoothing_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        "MATH.SMOOTH.".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_math_stack_keep_smooth(vm: &mut VM) -> Result<&mut VM, Error> {
    math_smoothing_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        "MATH.SMOOTH,".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_math_wb_keep_smooth(vm: &mut VM) -> Result<&mut VM, Error> {
    math_smoothing_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        "MATH.SMOOTH.,".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "math.smoothing".to_string(),
        stdlib_math_stack_consume_smooth,
    );
    let _ = vm
        .vm
        .register_inline("math.smoothing.".to_string(), stdlib_math_wb_consume_smooth);
    let _ = vm
        .vm
        .register_inline("math.smoothing,".to_string(), stdlib_math_stack_keep_smooth);
    let _ = vm
        .vm
        .register_inline("math.smoothing.,".to_string(), stdlib_math_wb_keep_smooth);

    Ok(())
}
