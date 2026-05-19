extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

pub fn stdlib_db_sync(vm: &mut VM) -> Result<&mut VM, Error> {
    match crate::globals::sync_db() {
        Ok(())   => vm.stack.push(Value::from_bool(true)),
        Err(err) => bail!("DB.SYNC returned: {}", err),
    };
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline("db.sync".to_string(), stdlib_db_sync)?;
    Ok(())
}
