use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use mathlab::functions::args;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use crate::vm::helpers;

pub enum Ops {
    Exp,
}

pub enum SeqOrd {
    Asc,
    Desc,
}

fn seq_ascending(vm: &mut VM, conf: Value) -> Value {
    let x = helpers::conf::conf_get(vm, conf.clone(), "X".to_string(), Value::from_float(0.0))
        .cast_float()
        .unwrap();
    let step =
        helpers::conf::conf_get(vm, conf.clone(), "Step".to_string(), Value::from_float(1.0))
            .cast_float()
            .unwrap();
    let n = helpers::conf::conf_get(vm, conf.clone(), "N".to_string(), Value::from_int(128))
        .cast_int()
        .unwrap();
    let fres = args::range(x, step, n as usize, "asc");
    let mut res: Vec<Value> = Vec::new();
    for v in fres {
        res.push(Value::from_float(v));
    }
    return Value::from_list(res);
}

fn seq_descending(vm: &mut VM, conf: Value) -> Value {
    let x = helpers::conf::conf_get(vm, conf.clone(), "X".to_string(), Value::from_float(0.0))
        .cast_float()
        .unwrap();
    let step =
        helpers::conf::conf_get(vm, conf.clone(), "Step".to_string(), Value::from_float(1.0))
            .cast_float()
            .unwrap();
    let n = helpers::conf::conf_get(vm, conf.clone(), "N".to_string(), Value::from_int(128))
        .cast_int()
        .unwrap();
    let fres = args::range(x, step, n as usize, "desc");
    let mut res: Vec<Value> = Vec::new();
    for v in fres {
        res.push(Value::from_float(v));
    }
    return Value::from_list(res);
}

fn seq_single(vm: &mut VM, conf: Value) -> Value {
    let x = helpers::conf::conf_get(vm, conf.clone(), "X".to_string(), Value::from_float(0.0))
        .cast_float()
        .unwrap();
    let n = helpers::conf::conf_get(vm, conf.clone(), "N".to_string(), Value::from_int(128))
        .cast_int()
        .unwrap();
    let fres = args::monolist(x, n as usize);
    let mut res: Vec<Value> = Vec::new();
    for v in fres {
        res.push(Value::from_float(v));
    }
    return Value::from_list(res);
}

pub fn stdlib_float_gen_seq_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for inline SEQ");
    }
    match vm.stack.pull() {
        Some(conf) => {
            let seq_type = helpers::conf::conf_get(
                vm,
                conf.clone(),
                "type".to_string(),
                Value::from_string("seq.ascending"),
            );
            let res = match seq_type.cast_string().unwrap().as_str() {
                "seq.ascending" => seq_ascending(vm, conf),
                "seq.descending" => seq_descending(vm, conf),
                "single" => seq_single(vm, conf),
                _ => bail!("Unknown SEQ type: {}", &seq_type),
            };
            vm.stack.push(res);
        }
        None => {
            bail!("SEQ_OP returns: NO DATA #1");
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_float_gen_seq_asc_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_gen_seq_inline(vm, SeqOrd::Asc)
}

#[time_graph::instrument]
pub fn stdlib_float_gen_seq_desc_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_math_float_gen_seq_inline(vm, SeqOrd::Desc)
}

#[time_graph::instrument]
pub fn stdlib_math_float_gen_seq_inline(vm: &mut VM, ord: SeqOrd) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 3 {
        bail!("Stack is too shallow for inline SEQ");
    }
    match vm.stack.pull() {
        Some(x_value) => match x_value.cast_float() {
            Ok(xvalue) => match vm.stack.pull() {
                Some(s_value) => match s_value.cast_float() {
                    Ok(step) => match vm.stack.pull() {
                        Some(n_value) => match n_value.cast_int() {
                            Ok(n) => {
                                let fres = match ord {
                                    SeqOrd::Asc => args::range(xvalue, step, n as usize, "asc"),
                                    SeqOrd::Desc => args::range(xvalue, step, n as usize, "desc"),
                                };
                                let mut res: Vec<Value> = Vec::new();
                                for v in fres {
                                    res.push(Value::from_float(v));
                                }
                                vm.stack.push(Value::from_list(res));
                            }
                            Err(err) => {
                                bail!("Casting N returns: {}", err);
                            }
                        },
                        None => {
                            bail!("SEQ_OP returns: NO DATA #3");
                        }
                    },
                    Err(err) => {
                        bail!("Casting Step returns: {}", err);
                    }
                },
                None => {
                    bail!("SEQ_OP returns: NO DATA #2");
                }
            },
            Err(err) => {
                bail!("Casting X returns: {}", err);
            }
        },
        None => {
            bail!("SEQ_OP returns: NO DATA #1");
        }
    }
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("seq".to_string(), stdlib_float_gen_seq_inline)?;
    let _ = vm
        .vm
        .register_inline("seq.asc".to_string(), stdlib_float_gen_seq_asc_inline)?;
    let _ = vm
        .vm
        .register_inline("seq.desc".to_string(), stdlib_float_gen_seq_desc_inline)?;
    Ok(())
}
