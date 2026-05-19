use rust_multistackvm::multistackvm::VM;

use bundcore::bundcore::Bund;
use easy_error::Error;
use fastrand::u64;
use lazy_static::lazy_static;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_mt::Mt64;
use rust_dynamic::value::Value;
use std::sync::Mutex;

lazy_static! {
    static ref RAND: Mutex<Mt64> = {
        let e: Mutex<Mt64> = Mutex::new(Mt64::new(u64(1..1000000000000)));
        e
    };
}

lazy_static! {
    static ref SEC_RAND: Mutex<ChaCha20Rng> = {
        let e: Mutex<ChaCha20Rng> = Mutex::new(ChaCha20Rng::from_os_rng());
        e
    };
}

#[time_graph::instrument]
pub fn stdlib_math_random_int_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    let mut rnd = RAND.lock().unwrap();
    let val = rnd.next_u64();
    drop(rnd);
    vm.stack.push(Value::from_int((val as i64).abs()));
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_math_random_chacha_int_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    let mut rnd = SEC_RAND.lock().unwrap();
    let val = rnd.next_u64();
    drop(rnd);
    vm.stack.push(Value::from_int((val as i64).abs()));
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let rnd = RAND.lock().unwrap();
    log::debug!("Initialize INT random generator");
    drop(rnd);
    let rnd = SEC_RAND.lock().unwrap();
    log::debug!("Initialize SECURE INT random generator");
    drop(rnd);
    let _ = vm
        .vm
        .register_inline("math.random.int".to_string(), stdlib_math_random_int_inline)?;
    let _ = vm.vm.register_inline(
        "math.securerandom.int".to_string(),
        stdlib_math_random_chacha_int_inline,
    )?;
    Ok(())
}
