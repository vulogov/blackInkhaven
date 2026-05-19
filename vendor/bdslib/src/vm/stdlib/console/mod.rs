extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod terminal;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    terminal::init_stdlib(vm)?;
    Ok(())
}
