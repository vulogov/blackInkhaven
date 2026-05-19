extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;
use rust_multistackvm::multistackvm::VM;
use std::process;

pub fn stdlib_bund_exit_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    if vm.stack.current_stack_len() == 0 {
        log::debug!("BUND is exiting error code 0. Stack is empty");
        process::exit(0);
    }
    let err_code_val = match vm.stack.pull() {
        Some(err_code_val) => err_code_val,
        None => {
            log::debug!("BUND is exiting error code 0. Can not get an error code");
            process::exit(0);
        }
    };
    let err_code = match err_code_val.cast_int() {
        Ok(err_code) => err_code,
        Err(err) => {
            log::error!("Error in casting error code for exit: {}", err);
            0 as i64
        }
    };
    log::debug!("BUND is exiting with code {}", err_code);
    process::exit(err_code as i32);
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("bund.exit".to_string(), stdlib_bund_exit_inline)?;
    Ok(())
}
