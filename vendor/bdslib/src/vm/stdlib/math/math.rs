extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use mathlab::math;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

pub enum Ops {
    Csc,
    Exp,
    Fact,
    Ln,
    Log10,
    Nroot,
    Perimeter,
    Power,
}

pub fn stdlib_float_csc_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Csc)
}

pub fn stdlib_float_exp_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Exp)
}

pub fn stdlib_float_fact_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Fact)
}

pub fn stdlib_float_ln_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Ln)
}

pub fn stdlib_float_log10_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Log10)
}

pub fn stdlib_float_nroot_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Nroot)
}

pub fn stdlib_float_perimeter_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Perimeter)
}

pub fn stdlib_float_pow_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_inline(vm, Ops::Power)
}

#[time_graph::instrument]
pub fn stdlib_math_float_inline(vm: &mut VM, op: Ops) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for inline float_op");
    }
    match vm.stack.pull() {
        Some(value) => match value.cast_float() {
            Ok(fvalue) => match op {
                Ops::Csc => {
                    vm.stack.push(Value::from_float(math::csc(fvalue)));
                }
                Ops::Exp => {
                    vm.stack.push(Value::from_float(math::exp(fvalue)));
                }
                Ops::Fact => {
                    vm.stack
                        .push(Value::from_float(math::fact(fvalue as u64) as f64));
                }
                Ops::Ln => {
                    vm.stack.push(Value::from_float(math::ln(fvalue)));
                }
                Ops::Log10 => {
                    vm.stack.push(Value::from_float(math::log10(fvalue)));
                }
                Ops::Nroot => match vm.stack.pull() {
                    Some(value2) => match value2.cast_float() {
                        Ok(nvalue) => {
                            vm.stack.push(Value::from_float(math::nrt(fvalue, nvalue)));
                        }
                        Err(err) => {
                            bail!("FLOAT_OP returns error: {}", err);
                        }
                    },
                    None => {
                        bail!("FLOAT_OP returns: NO DATA #2");
                    }
                },
                Ops::Perimeter => match vm.stack.pull() {
                    Some(value2) => match value2.cast_float() {
                        Ok(yvalue) => {
                            vm.stack
                                .push(Value::from_float(math::perimeter(fvalue, yvalue)));
                        }
                        Err(err) => {
                            bail!("FLOAT_OP returns error: {}", err);
                        }
                    },
                    None => {
                        bail!("FLOAT_OP returns: NO DATA #2");
                    }
                },
                Ops::Power => match vm.stack.pull() {
                    Some(value2) => match value2.cast_float() {
                        Ok(xvalue) => {
                            vm.stack.push(Value::from_float(math::pow(xvalue, fvalue)));
                        }
                        Err(err) => {
                            bail!("FLOAT_OP returns error: {}", err);
                        }
                    },
                    None => {
                        bail!("FLOAT_OP returns: NO DATA #2");
                    }
                },
            },
            Err(err) => {
                bail!("FLOAT_OP returns error: {}", err);
            }
        },
        None => {
            bail!("FLOAT_OP returns: NO DATA #1");
        }
    }
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("math.cosecant".to_string(), stdlib_float_csc_inline)?;
    let _ = vm
        .vm
        .register_inline("math.exp".to_string(), stdlib_float_exp_inline)?;
    let _ = vm
        .vm
        .register_inline("math.factorial".to_string(), stdlib_float_fact_inline)?;
    let _ = vm
        .vm
        .register_inline("math.ln".to_string(), stdlib_float_ln_inline)?;
    let _ = vm
        .vm
        .register_inline("math.log10".to_string(), stdlib_float_log10_inline)?;
    let _ = vm
        .vm
        .register_inline("math.nroot".to_string(), stdlib_float_nroot_inline)?;
    let _ = vm
        .vm
        .register_inline("math.perimeter".to_string(), stdlib_float_perimeter_inline)?;
    let _ = vm
        .vm
        .register_inline("math.power".to_string(), stdlib_float_pow_inline)?;
    Ok(())
}
