extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod db;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    db::init_stdlib(vm)?;
    Ok(())
}
