extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod base64;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    base64::init_stdlib(vm)?;
    Ok(())
}
