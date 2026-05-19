extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

pub fn stdlib_is_global_db(vm: &mut VM) -> Result<&mut VM, Error> {
    match crate::globals::get_db().is_ok() {
        true => vm.stack.push(Value::from_bool(true)),
        false => vm.stack.push(Value::from_bool(false)),
    };
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("?db".to_string(), stdlib_is_global_db);
    Ok(())
}
