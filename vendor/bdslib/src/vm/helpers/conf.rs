use rust_multistackvm::multistackvm::{VM};
use rust_dynamic::value::Value;

#[time_graph::instrument]
pub fn conf_get(_vm: &mut VM, vconf: Value, key: String, default: Value) -> Value  {
    let conf = match vconf.cast_dict() {
        Ok(conf) => conf,
        Err(err) => {
            log::error!("Error casting conf object: {}", err);
            return default;
        }
    };
    if conf.contains_key(&key) {
        let res = match conf.get(&key) {
            Some(res) => res,
            None => &default,
        };
        return res.clone();
    } else {
        return default;
    }
}
