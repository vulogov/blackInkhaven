extern crate log;
use env_logger::Env;

pub fn setloglevel(c: u32) {
    let env = Env::default().filter_or("BDS_LOG_LEVEL", "error");
    match c {
        0 => {
            env_logger::init_from_env(env);
            log::debug!("Set loglevel from environment");
        }
        1 => {
            let env = Env::default().filter_or("BDS_LOG_LEVEL", "bdslib,bdscli,bdsnode=info");
            env_logger::init_from_env(env);
            log::debug!("Set loglevel=info");
        }
        2 => {
            let env = Env::default().filter_or("BDS_LOG_LEVEL", "bdslib,bdscli,bdsnode=debug");
            env_logger::init_from_env(env);
            log::debug!("Set loglevel=debug");
        }
        _ => {
            let env = Env::default().filter_or("BDS_LOG_LEVEL", "bdslib,bdscli,bdsnode=trace");
            env_logger::init_from_env(env);
            log::debug!("Set loglevel=trace");
        }
    }
    log::debug!("setloglevel::setloglevel() reached")
}
