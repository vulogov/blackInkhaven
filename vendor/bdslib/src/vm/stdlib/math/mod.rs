extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod anomalies;
pub mod breakout;
pub mod clusters;
pub mod interp;
pub mod math;
pub mod normalize;
pub mod rand;
pub mod seq;
pub mod smoothing;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    normalize::init_stdlib(vm)?;
    smoothing::init_stdlib(vm)?;
    math::init_stdlib(vm)?;
    seq::init_stdlib(vm)?;
    rand::init_stdlib(vm)?;
    interp::init_stdlib(vm)?;
    Ok(())
}
