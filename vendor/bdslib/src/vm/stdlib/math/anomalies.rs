use anomaly_detection;
use rust_dynamic::value::Value;
use augurs::outlier::{MADDetector, DbscanDetector, OutlierDetector};
use easy_error::{Error, bail};

pub fn detect_anomalies(source: Vec<f64>, period: usize) -> Result<Vec<f64>, Error> {
    let mut src: Vec<f32> = Vec::new();
    let mut dst: Vec<f64> = Vec::new();
    for i in &source {
        src.push(*i as f32);
    }
    let _ = match anomaly_detection::AnomalyDetector::fit(&src, period) {
        Ok(res_ix) => {
            for ix in res_ix.anomalies() {
                dst.push(src[*ix].into());
            }
        }
        Err(err) => bail!("ANOMALY DETECTION returns error: {}", err),
    };
    Ok(dst)
}

pub fn detect_outliers(source1: Vec<f64>, source2: Vec<f64>, sensitivity: f64) -> Result<Value, Error> {
    let series: &[&[f64]] = &[
        source1.as_slice(),
        source2.as_slice(),
    ];
    let detector = match MADDetector::with_sensitivity(sensitivity) {
        Ok(detector) => detector,
        Err(err) => bail!("DETECT_OUTLIERS: detector create returns: {}", err),
    };
    let processed = match detector.preprocess(&series) {
        Ok(processed) => processed,
        Err(err) => bail!("DETECT_OUTLIERS: detector processor returns: {}", err),
    };
    let outliers = match detector.detect(&processed) {
        Ok(outliers) => outliers,
        Err(err) => bail!("DETECT_OUTLIERS: detector detector returns: {}", err),
    };
    let mut s = 0;
    let mut res = Value::dict();
    for r in outliers.series_results {
        let mut row  = Value::list();
        for i in r.outlier_intervals.intervals {
            let mut data_set = Value::list();
            let start = i.start;
            let end = match i.end {
                Some(end) => end,
                None => continue,
            };
            for v in &series[s][start..end] {
                data_set = data_set.push(Value::from_float(*v));
            }
            row = row.push(data_set);
        }
        res = res.set(format!("{}", s), row);
        s = s + 1
    }
    Ok(res)
}

pub fn detect_outliers_dbscan(source1: Vec<f64>, source2: Vec<f64>, sensitivity: f64) -> Result<Value, Error> {
    let series: &[&[f64]] = &[
        source1.as_slice(),
        source2.as_slice(),
    ];
    let detector = match DbscanDetector::with_sensitivity(sensitivity) {
        Ok(detector) => detector,
        Err(err) => bail!("DETECT_OUTLIERS: detector create returns: {}", err),
    };
    let processed = match detector.preprocess(&series) {
        Ok(processed) => processed,
        Err(err) => bail!("DETECT_OUTLIERS: detector processor returns: {}", err),
    };
    let outliers = match detector.detect(&processed) {
        Ok(outliers) => outliers,
        Err(err) => bail!("DETECT_OUTLIERS: detector detector returns: {}", err),
    };
    let mut s = 0;
    let mut res = Value::dict();
    for r in outliers.series_results {
        let mut row  = Value::list();
        for i in r.outlier_intervals.intervals {
            let mut data_set = Value::list();
            let start = i.start;
            let end = match i.end {
                Some(end) => end,
                None => continue,
            };
            for v in &series[s][start..end] {
                data_set = data_set.push(Value::from_float(*v));
            }
            row = row.push(data_set);
        }
        res = res.set(format!("{}", s), row);
        s = s + 1
    }
    Ok(res)
}
