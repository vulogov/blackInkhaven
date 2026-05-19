extern crate log;

use crate::vm::stdlib::init_bund_stdlib;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use parking_lot::RwLock;

pub fn init_adam() -> Result<(), Error> {
    let mut adam = Bund::new();
    init_stdlib(&mut adam)?;
    match crate::vm::BUND.get().is_some() {
        true => log::info!("BUND Adam instance already initialized."),
        false => match crate::vm::BUND.set(RwLock::new(adam)) {
            Ok(_) => {
                log::debug!("BUND Adam instance succesfully initialized.")
            }
            Err(err) => bail!("Error initializing BUND Adam instance: {:?}", err),
        },
    }
    Ok(())
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    init_bund_stdlib(vm)
}
