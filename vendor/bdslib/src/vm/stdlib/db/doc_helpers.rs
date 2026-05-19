extern crate log;

use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use uuid::Uuid;

/// Pull one value from the stack or workbench according to `op`.
pub fn pull(vm: &mut VM, op: &StackOps) -> Option<Value> {
    match op {
        StackOps::FromStack     => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    }
}

/// Push one value to the stack or workbench according to `op`.
pub fn push(vm: &mut VM, op: &StackOps, v: Value) {
    let _ = match op {
        StackOps::FromStack     => vm.stack.push(v),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(v),
    };
}

/// Assert minimum depth for the chosen source.
pub fn require_depth(vm: &mut VM, op: &StackOps, n: usize, err_prefix: &str) -> Result<(), Error> {
    let depth = match op {
        StackOps::FromStack     => vm.stack.current_stack_len(),
        StackOps::FromWorkBench => vm.stack.workbench.len(),
    };
    if depth < n {
        bail!("{} requires {} item(s) but only {} available", err_prefix, n, depth);
    }
    Ok(())
}

/// Parse a UUID from a string `Value`.
pub fn value_to_uuid(v: Value, err_prefix: &str) -> Result<Uuid, Error> {
    let s = match v.cast_string() {
        Ok(s)    => s,
        Err(err) => bail!("{} UUID string cast failed: {}", err_prefix, err),
    };
    match Uuid::parse_str(&s) {
        Ok(id)   => Ok(id),
        Err(err) => bail!("{} UUID parse failed: {}", err_prefix, err),
    }
}

/// Convert a LIST `Value` of floats to `Vec<f32>`.
pub fn value_to_f32_vec(v: Value, err_prefix: &str) -> Result<Vec<f32>, Error> {
    let items = match v.cast_list() {
        Ok(items) => items,
        Err(err)  => bail!("{} vector list cast failed: {}", err_prefix, err),
    };
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        match item.cast_float() {
            Ok(f)    => result.push(f as f32),
            Err(err) => bail!("{} vector element cast failed: {}", err_prefix, err),
        }
    }
    Ok(result)
}

/// Extract a string `Value` as `String`.
pub fn value_to_string(v: Value, field: &str, err_prefix: &str) -> Result<String, Error> {
    match v.cast_string() {
        Ok(s)    => Ok(s),
        Err(err) => bail!("{} {} string cast failed: {}", err_prefix, field, err),
    }
}
