extern crate log;

use easy_error::{Error, bail};
use leon::Template;
use rust_dynamic::types::*;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use std::collections::HashMap;

fn render_template(vm: &mut VM, fmt_value: Value, value: Value) -> Result<String, Error> {
    let str_tpl = match value.cast_string() {
        Ok(str_tpl) => str_tpl,
        Err(err) => bail!("FMT.STR error casting template: {}", err),
    };
    let locale = match sys_locale::get_locale() {
        Some(loc) => loc,
        None => "en-US".to_string(),
    };
    let template = match Template::parse(str_tpl.as_str()) {
        Ok(template) => template,
        Err(err) => {
            bail!("FMT.STR error parsing template: {}", err);
        }
    };
    let mut values: HashMap<String, String> = HashMap::new();
    'outer: for name in template.keys() {
        if values.contains_key(&name.to_string().clone()) {
            continue;
        }
        for n in vec![format!("{}.{}", &name, &locale), name.to_string()] {
            if fmt_value.has_key(&n).unwrap().cast_bool().unwrap() {
                let inner_value = match fmt_value.get(&n) {
                    Ok(inner_value) => inner_value,
                    Err(err) => bail!("FMT.STR error getting object key: {}", err),
                };
                match conditional_fmt_str(vm, fmt_value.clone(), inner_value) {
                    Ok(val) => {
                        values.insert(n.to_string(), val);
                    }
                    Err(err) => {
                        bail!("FMT.STR error processing object key: {}", err);
                    }
                }
                continue 'outer;
            }
        }
        match vm.stack.pull() {
            Some(value) => match value.conv(STRING) {
                Ok(str_val) => match str_val.cast_string() {
                    Ok(val) => {
                        values.insert(name.to_string(), val);
                    }
                    Err(err) => {
                        bail!("FMT.STR error casting: {}", err);
                    }
                },
                Err(err) => {
                    bail!("FMT.STR error converting: {}", err);
                }
            },
            None => {
                bail!("FMT.STR: stack is too shallow");
            }
        }
    }
    let res = match template.render(&values) {
        Ok(res) => res.to_string(),
        Err(err) => {
            bail!("FMT.STR error rendering: {}", err);
        }
    };
    Ok(res)
}

pub fn conditional_fmt_fmt_str(
    vm: &mut VM,
    fmt_value: Value,
    value: Value,
) -> Result<String, Error> {
    if value.type_of() == STRING {
        match render_template(vm, fmt_value, value) {
            Ok(str_value) => {
                return Ok(str_value);
            }
            Err(err) => {
                bail!("FMT.STR: rendering template returned error: {}", err);
            }
        }
    } else {
        match value.conv(STRING) {
            Ok(str_value) => {
                return Ok(str_value.cast_string().unwrap());
            }
            Err(err) => {
                bail!("FMT.STR: conversion to STRING returned error: {}", err);
            }
        }
    }
}

pub fn conditional_fmt_str(vm: &mut VM, fmt_value: Value, value: Value) -> Result<String, Error> {
    match fmt_value.dt {
        CONDITIONAL => {
            let fmt_type_val = match fmt_value.get("type") {
                Ok(fmt_type_val) => fmt_type_val,
                Err(err) => bail!("FMT.STR: getting fmt type returns error: {}", err),
            };
            let fmt_type = match fmt_type_val.cast_string() {
                Ok(fmt_type) => fmt_type,
                Err(err) => bail!("FMT.STR: casting fmt type returns error: {}", err),
            };
            match fmt_type.as_str() {
                "fmt" => {
                    return conditional_fmt_fmt_str(vm, fmt_value, value);
                }
                _ => {
                    bail!("FMT.STR: conditional is of incorrect type: {}", &fmt_type);
                }
            }
        }
        _ => match value.conv(STRING) {
            Ok(str_value) => {
                return Ok(str_value.cast_string().unwrap());
            }
            Err(err) => {
                bail!("FMT.STR: conversion to STRING returned error: {}", err);
            }
        },
    }
}

pub fn stdlib_conditional_fmt(vm: &mut VM) -> Result<&mut VM, Error> {
    let mut res: Value = Value::conditional();
    res = res.set("type", Value::from_string("fmt"));
    vm.stack.push(res);
    Ok(vm)
}

pub fn conditional_run(vm: &mut VM, value: Value) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for FMT.RUN");
    }
    let name_val = match vm.stack.pull() {
        Some(name_val) => name_val,
        None => bail!("FMT.RUN: No context name discovered on the stack"),
    };
    let name = match name_val.cast_string() {
        Ok(name) => name,
        Err(err) => bail!("FMT.RUN: Error name casting: {}", err),
    };
    let locale = match sys_locale::get_locale() {
        Some(loc) => loc,
        None => "en-US".to_string(),
    };
    let msg_val = match value.get(format!("{}.{}", &name, &locale).to_string()) {
        Ok(msg_val) => msg_val,
        Err(_) => match value.get(format!("{}", &name)) {
            Ok(msg_val) => msg_val,
            Err(err) => bail!(
                "FMT.RUN: getting message with name {} returns error: {}",
                &name,
                err
            ),
        },
    };
    match conditional_fmt_str(vm, value, msg_val) {
        Ok(str_val) => {
            vm.stack.push(Value::from_string(str_val));
        }
        Err(err) => {
            bail!(
                "FMT.RUN: error converting message with name {} returns error: {}",
                &name,
                err
            );
        }
    }
    Ok(vm)
}
