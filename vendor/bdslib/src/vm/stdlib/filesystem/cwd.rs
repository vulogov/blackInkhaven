extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use std::env::current_dir;

pub fn stdlib_bund_fs_cwd_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    match current_dir() {
        Ok(cwd) => {
            vm.stack
                .push(Value::from_string(format!("{}", cwd.display())));
        }
        Err(err) => {
            bail!("FS.CWD returned: {}", err);
        }
    }
    Ok(vm)
}

pub fn stdlib_bund_fs_cwd_disabled(_vm: &mut VM) -> Result<&mut VM, Error> {
    bail!("bund FS.CWD functions disabled with --noio");
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("fs.cwd".to_string(), stdlib_bund_fs_cwd_inline);
    Ok(())
}
