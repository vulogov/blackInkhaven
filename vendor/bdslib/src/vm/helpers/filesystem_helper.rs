extern crate log;
use filesystem::{FileSystem, OsFileSystem};

#[time_graph::instrument]
pub fn filesystem_is_file(path: String) -> bool {
    let fs = OsFileSystem::new();
    fs.is_file(path)
}
