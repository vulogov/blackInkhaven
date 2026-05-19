extern crate log;

use crate::vm::stdlib::statistics;
use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

fn math_normalize_base(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match statistics::get_data::get_data(vm, op.clone(), smode, err_prefix.clone()) {
        Ok(source) => {
            let min_sample = source.iter().cloned().fold(0. / 0., f64::min);
            let max_sample = source.iter().cloned().fold(0. / 0., f64::max);
            let mut res = Value::list();
            if max_sample - min_sample == 0.0 {
                for v in source {
                    res = res.push(Value::from_float(v));
                }
            } else {
                for v in source {
                    res = res.push(Value::from_float(
                        (v - min_sample) / (max_sample - min_sample) as f64,
                    ));
                }
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
pub fn stdlib_math_stack_consume_normalize(vm: &mut VM) -> Result<&mut VM, Error> {
    math_normalize_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        "MATH.NORMALIZE".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_math_wb_consume_normalize(vm: &mut VM) -> Result<&mut VM, Error> {
    math_normalize_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        "MATH.NORMALIZE.".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_math_stack_keep_normalize(vm: &mut VM) -> Result<&mut VM, Error> {
    math_normalize_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        "MATH.NORMALIZE,".to_string(),
    )
}

#[time_graph::instrument]
pub fn math_math_wb_keep_normalize(vm: &mut VM) -> Result<&mut VM, Error> {
    math_normalize_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        "MATH.NORMALIZE.,".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "math.normalize".to_string(),
        stdlib_math_stack_consume_normalize,
    )?;
    let _ = vm.vm.register_inline(
        "math.normalize.".to_string(),
        stdlib_math_wb_consume_normalize,
    )?;
    let _ = vm.vm.register_inline(
        "math.normalize,".to_string(),
        stdlib_math_stack_keep_normalize,
    );
    let _ = vm
        .vm
        .register_inline("math.normalize.,".to_string(), math_math_wb_keep_normalize)?;

    Ok(())
}
