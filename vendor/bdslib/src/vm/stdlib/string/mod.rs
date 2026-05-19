extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod any_id;
pub mod distance;
pub mod fuzzy_match;
pub mod grok;
pub mod prefix_suffix;
pub mod random;
pub mod regex;
pub mod regex_matches;
pub mod regex_split;
pub mod textexpr_match;
pub mod textwrap;
pub mod tokenize;
pub mod unicode;
pub mod wildmatch;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    wildmatch::init_stdlib(vm)?;
    fuzzy_match::init_stdlib(vm)?;
    distance::init_stdlib(vm)?;
    regex::init_stdlib(vm)?;
    regex_matches::init_stdlib(vm)?;
    regex_split::init_stdlib(vm)?;
    prefix_suffix::init_stdlib(vm)?;
    any_id::init_stdlib(vm)?;
    tokenize::init_stdlib(vm)?;
    grok::init_stdlib(vm)?;
    random::init_stdlib(vm)?;
    unicode::init_stdlib(vm)?;
    textexpr_match::init_stdlib(vm)?;
    textwrap::init_stdlib(vm)?;
    Ok(())
}
