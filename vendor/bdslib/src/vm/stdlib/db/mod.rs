extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod db_add;
pub mod db_aggregation_search;
pub mod db_fulltext;
pub mod db_search;
pub mod db_sync;

pub mod doc_helpers;
pub mod doc_add;
pub mod doc_delete;
pub mod doc_get;
pub mod doc_search;
pub mod doc_search_strings;
pub mod doc_sync;
pub mod doc_update;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    // telemetry / shard DB words
    db_add::init_stdlib(vm)?;
    db_aggregation_search::init_stdlib(vm)?;
    db_search::init_stdlib(vm)?;
    db_fulltext::init_stdlib(vm)?;
    db_sync::init_stdlib(vm)?;
    // document store words
    doc_add::init_stdlib(vm)?;
    doc_update::init_stdlib(vm)?;
    doc_delete::init_stdlib(vm)?;
    doc_get::init_stdlib(vm)?;
    doc_search::init_stdlib(vm)?;
    doc_search_strings::init_stdlib(vm)?;
    doc_sync::init_stdlib(vm)?;
    Ok(())
}
