use sys_metrics::host;

#[time_graph::instrument]
pub fn get_hostname() -> String  {
    match host::get_hostname() {
        Ok(hostname) => hostname,
        Err(_) => "local".to_string(),
    }
}
