extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

#[derive(Debug, Clone)]
pub enum SourceMode {
    Consume,
    Keep,
}

pub mod get_data;

pub mod count;
pub mod minmax;
pub mod statistics;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    count::init_stdlib(vm)?;
    statistics::init_stdlib(vm)?;
    minmax::init_stdlib(vm)?;
    Ok(())
}
