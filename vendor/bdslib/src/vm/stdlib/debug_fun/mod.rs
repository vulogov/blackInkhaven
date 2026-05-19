extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod debug_display_stack;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    debug_display_stack::init_stdlib(vm)?;
    Ok(())
}
