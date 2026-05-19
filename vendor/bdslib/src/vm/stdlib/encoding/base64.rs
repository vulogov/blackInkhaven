extern crate log;

use base64ct::{Base64, Encoding};
use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

fn bund_encode_base64_base(
    vm: &mut VM,
    op: StackOps,
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
    let object = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let object_val = match object {
        Some(object_val) => object_val,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let data = match object_val.to_binary() {
        Ok(data) => data,
        Err(err) => {
            bail!("{} wrapping object returned: {}", &err_prefix, err);
        }
    };
    let encoded = Base64::encode_string(data.as_ref());
    let encoded_val = Value::from_string(encoded);
    let _ = match op {
        StackOps::FromStack => vm.stack.push(encoded_val),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(encoded_val),
    };
    return Ok(vm);
}

fn bund_decode_base64_base(
    vm: &mut VM,
    op: StackOps,
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
    let object = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let object_val = match object {
        Some(object_val) => object_val,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let string_data = match object_val.cast_string() {
        Ok(string_val) => string_val,
        Err(err) => {
            bail!("{} casting object returned: {}", &err_prefix, err);
        }
    };
    let bin_data = match Base64::decode_vec(string_data.as_ref()) {
        Ok(bin_data) => bin_data,
        Err(err) => {
            bail!("{} decoding object returned: {}", &err_prefix, err);
        }
    };
    let result_value = match Value::from_binary(bin_data) {
        Ok(data) => data,
        Err(err) => {
            bail!("{} unwrapping object returned: {}", &err_prefix, err);
        }
    };
    let _ = match op {
        StackOps::FromStack => vm.stack.push(result_value),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(result_value),
    };
    return Ok(vm);
}

pub fn stdlib_encode_base64_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_encode_base64_base(vm, StackOps::FromStack, "ENCODE.BASE64".to_string())
}

pub fn stdlib_encode_base64_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_encode_base64_base(vm, StackOps::FromWorkBench, "ENCODE.BASE64.".to_string())
}

pub fn stdlib_decode_base64_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_decode_base64_base(vm, StackOps::FromStack, "DECODE.BASE64".to_string())
}

pub fn stdlib_decode_base64_workbench(vm: &mut VM) -> Result<&mut VM, Error> {
    bund_decode_base64_base(vm, StackOps::FromWorkBench, "DECODE.BASE64.".to_string())
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("encode.base64".to_string(), stdlib_encode_base64_stack);
    let _ = vm
        .vm
        .register_inline("encode.base64.".to_string(), stdlib_encode_base64_workbench);
    let _ = vm
        .vm
        .register_inline("decode.base64".to_string(), stdlib_decode_base64_stack);
    let _ = vm
        .vm
        .register_inline("decode.base64.".to_string(), stdlib_decode_base64_workbench);
    Ok(())
}
