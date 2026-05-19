extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

#[time_graph::instrument]
pub fn stdlib_bund_is_alias(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for ?ALIAS");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("?ALIAS returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("?ALIAS casting string returns: {}", err);
        }
    };
    let res = Value::from_bool(vm.is_alias(fn_name));
    vm.stack.push(res);
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_is_lambda(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for ?LAMBDA");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("?LAMBDA returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("?LAMBDA casting string returns: {}", err);
        }
    };
    let res = Value::from_bool(vm.is_lambda(fn_name));
    vm.stack.push(res);
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_is_stdlib(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for ?STDLIB");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("?STDLIB returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("?STDLIB casting string returns: {}", err);
        }
    };
    let res = Value::from_bool(vm.is_inline(fn_name));
    vm.stack.push(res);
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_is_callable(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for ?WORD");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("?WORD returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("?WORD casting string returns: {}", err);
        }
    };
    let res_bool = if vm.is_inline(fn_name.clone()) {
        true
    } else if vm.is_lambda(fn_name.clone()) {
        true
    } else if vm.is_alias(fn_name.clone()) {
        true
    } else {
        false
    };
    let res = Value::from_bool(res_bool);
    vm.stack.push(res);
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_get_alias(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for ?ALIAS");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("?ALIAS.GET returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("?ALIAS.GET casting string returns: {}", err);
        }
    };
    match vm.get_alias(fn_name) {
        Ok(fn_alias) => {
            vm.stack.push(Value::from_string(fn_alias));
        }
        Err(err) => {
            bail!("ALIAS.GET returned: {}", err);
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_get_lambda(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for LAMBDA.GET");
    }
    let fn_name_value = match vm.stack.pull() {
        Some(fn_name_value) => fn_name_value,
        None => {
            bail!("LAMBDA.GET returns NO DATA #1");
        }
    };
    let fn_name = match fn_name_value.cast_string() {
        Ok(fn_name) => fn_name,
        Err(err) => {
            bail!("LAMBDA.GET casting string returns: {}", err);
        }
    };
    match vm.get_lambda(fn_name) {
        Ok(fn_value) => {
            vm.stack.push(fn_value);
        }
        Err(err) => {
            bail!("LAMBDA.GET returned: {}", err);
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_to_lambda(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() < 1 {
        bail!("Stack is too shallow for LAMBDA.MAKE");
    }
    let list_value = match vm.stack.pull() {
        Some(list_value) => list_value,
        None => {
            bail!("LAMBDA.MAKE returns NO DATA #1");
        }
    };
    match list_value.cast_list() {
        Ok(data) => {
            let mut lambda_raw: Vec<Value> = Vec::new();
            for v in data {
                lambda_raw.push(v);
            }
            vm.stack.push(Value::to_lambda(lambda_raw));
        }
        Err(err) => {
            bail!("LAMBDA.MAKE casting list returned: {}", err);
        }
    }
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_bund_fold_lambda(vm: &mut VM) -> Result<&mut VM, Error> {
    let mut lambda_raw: Vec<Value> = Vec::new();
    loop {
        let value = match vm.stack.pull() {
            Some(value) => value,
            None => {
                break;
            }
        };
        lambda_raw.insert(0, value);
    }
    vm.stack.push(Value::to_lambda(lambda_raw));
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("?alias".to_string(), stdlib_bund_is_alias)?;
    let _ = vm
        .vm
        .register_inline("?lambda".to_string(), stdlib_bund_is_lambda)?;
    let _ = vm
        .vm
        .register_inline("?stdlib".to_string(), stdlib_bund_is_stdlib)?;
    let _ = vm
        .vm
        .register_inline("?word".to_string(), stdlib_bund_is_callable)?;
    let _ = vm
        .vm
        .register_inline("alias=".to_string(), stdlib_bund_get_alias)?;
    let _ = vm
        .vm
        .register_inline("lambda=".to_string(), stdlib_bund_get_lambda)?;
    let _ = vm
        .vm
        .register_inline("lambda!".to_string(), stdlib_bund_to_lambda)?;
    let _ = vm
        .vm
        .register_inline("lambda*".to_string(), stdlib_bund_fold_lambda)?;
    Ok(())
}
