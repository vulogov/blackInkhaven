extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod bund_eval;
pub mod bund_exit;
pub mod bund_fun;
pub mod bund_interpreter;
pub mod bund_use;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    bund_eval::init_stdlib(vm)?;
    bund_exit::init_stdlib(vm)?;
    bund_fun::init_stdlib(vm)?;
    bund_interpreter::init_stdlib(vm)?;
    bund_use::init_stdlib(vm)?;
    Ok(())
}
