extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod bund;
pub mod conditional;
pub mod console;
pub mod db;
pub mod debug_fun;
pub mod encoding;
pub mod filesystem;
pub mod global_db;
pub mod math;
pub mod statistics;
pub mod string;
pub mod system;

pub fn init_bund_stdlib(vm: &mut Bund) -> Result<(), Error> {
    console::init_stdlib(vm)?;
    bund::init_stdlib(vm)?;
    filesystem::init_stdlib(vm)?;
    conditional::init_stdlib(vm)?;
    system::init_stdlib(vm)?;
    global_db::init_stdlib(vm)?;
    db::init_stdlib(vm)?;
    string::init_stdlib(vm)?;
    statistics::init_stdlib(vm)?;
    math::init_stdlib(vm)?;
    debug_fun::init_stdlib(vm)?;
    Ok(())
}
