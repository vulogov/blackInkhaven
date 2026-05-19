extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;
use rust_multistackvm::stdlib::execute_types::CF;

pub mod conditional_fmt;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let mut cf = CF.lock().unwrap();

    cf.insert("fmt".to_string(), conditional_fmt::conditional_run);

    drop(cf);

    let _ = vm
        .vm
        .register_inline("fmt".to_string(), conditional_fmt::stdlib_conditional_fmt);
    Ok(())
}
