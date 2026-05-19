//! `inkhaven restore <backup> --to <dir>` — unpack a zipped project.
//! Refuses to overwrite an existing project so users can't lose work by
//! mistyping the destination.

use std::path::Path;

use crate::backup;
use crate::error::Result;

pub fn run(archive: &Path, to: &Path) -> Result<()> {
    if !archive.is_file() {
        return Err(crate::error::Error::Store(format!(
            "restore: archive `{}` not found",
            archive.display()
        )));
    }
    backup::restore_backup(archive, to)?;
    eprintln!("restored backup `{}` into {}", archive.display(), to.display());
    Ok(())
}
