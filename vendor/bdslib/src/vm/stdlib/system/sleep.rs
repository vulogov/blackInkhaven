extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_multistackvm::multistackvm::VM;
use spin_sleep;
use std::time::Duration;

pub fn stdlib_sleep_seconds(vm: &mut VM) -> Result<&mut VM, Error> {
    let n = match vm.stack.pull() {
        Some(n) => match n.cast_int() {
            Ok(n) => n,
            Err(err) => bail!("SLEEP::SECONDS error casting seconds: {}", err),
        },
        None => bail!("SLEEP::SECONDS: NO DATA #1"),
    };
    spin_sleep::sleep(Duration::new(n as u64, 0));
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("sleep.seconds".to_string(), stdlib_sleep_seconds);

    Ok(())
}
