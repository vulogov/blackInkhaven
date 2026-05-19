extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use ulid::Ulid;
use uuid::Uuid;

#[time_graph::instrument]
pub fn stdlib_uuid_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    vm.stack
        .push(Value::from_string(Uuid::new_v4().to_string()));
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_ulid_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    let ulid = Ulid::new();
    vm.stack.push(Value::from_string(ulid.to_string()));
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("id.uuid".to_string(), stdlib_uuid_inline)?;
    let _ = vm
        .vm
        .register_inline("id.ulid".to_string(), stdlib_ulid_inline)?;
    Ok(())
}
