use breakout;
use easy_error::{Error, bail};

pub fn detect_breakouts(source: Vec<f64>, n: usize) -> Result<Vec<f64>, Error> {
    let mut dst: Vec<f64> = Vec::new();
    let _ = match breakout::multi()
        .min_size(n)
        .degree(2)
        .beta(0.008)
        .percent(None)
        .fit(&source) {
        Ok(res_ix) => {
            for ix in res_ix {
                dst.push(source[ix].into());
            }
        }
        Err(err) => bail!("BREAKOUT DETECTION returns error: {}", err),
    };
    Ok(dst)
}
