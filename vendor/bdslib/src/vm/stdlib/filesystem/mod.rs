extern crate log;
use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod cp;
pub mod cwd;
pub mod file;
pub mod file_write;
pub mod filepath;
pub mod filesystem;
pub mod ls;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    filepath::init_stdlib(vm)?;
    filesystem::init_stdlib(vm)?;
    file::init_stdlib(vm)?;
    cp::init_stdlib(vm)?;
    ls::init_stdlib(vm)?;
    cwd::init_stdlib(vm)?;
    file_write::init_stdlib(vm)?;
    Ok(())
}
