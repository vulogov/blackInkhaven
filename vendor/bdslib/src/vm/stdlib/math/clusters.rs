use rust_dynamic::value::Value;
use augurs::{
    clustering::{DbscanClusterer},
    dtw::Dtw,
};
use easy_error::{Error};


pub fn detect_clusters(source1: Vec<f64>, source2: Vec<f64>, epsilon: f64) -> Result<Value, Error> {
    let series: &[&[f64]] = &[
        source1.as_slice(),
        source2.as_slice(),
    ];
    let distance_matrix = Dtw::euclidean()
                            .distance_matrix(series);
    let min_cluster_size = 2;

    let clusters = DbscanClusterer::new(epsilon, min_cluster_size).fit(&distance_matrix);

    let mut res = Value::list();
    for v in clusters {
        if v.is_cluster() {
            res = res.push(Value::from_int(v.as_i32() as i64));
        } else {
            res = res.push(Value::from_string("NOISE"));
        }
    }
    Ok(res)
}
